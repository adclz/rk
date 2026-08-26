use db::WorkspaceDataBase;
use hir::{
    hir_def::expressions::{
        expression::{
            AddOperatorKind, BooleanOperatorKind, ComparisonOperatorKind, Elementary, Expr,
            ExprKind, MultOperatorKind, ParamAssignKind, PathExprKind, PrimaryExpr, RefValue,
            UnaryOperatorKind, VarAccess, VariableAccess, VariableAccessKind,
        },
        invocation::InvocationKind,
        spec::ElementarySpec,
        statement::CaseKind,
    },
    hir_ty::{infer::Infer, ty::Type},
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    expr::{MirArgKind, MirBinOp, MirCall, MirCallArg, MirConstant, MirExpr, MirPlace, MirUnaryOp},
    lower::lower_type::{LowerTypeError, elementary_spec_to_mir, lower_type},
    stmt::MirCasePattern,
    types::{MirElementary, MirType},
};

/// Context for expression lowering, carrying shared state.
pub struct ExprLowerCtx<'db> {
    pub db: &'db dyn WorkspaceDataBase,
    /// The `this` struct when lowering an FB body: member accesses become
    /// `ThisField`.
    pub this_struct: Option<crate::types::MirStructType>,
    /// String literal pool - shared across all functions in the module.
    pub string_pool: std::rc::Rc<std::cell::RefCell<StringPool>>,
    /// Phase B: in a specialized body (`drive$Worker`), each interface param's
    /// concrete POU.
    pub iface_subs: Option<
        std::rc::Rc<
            rustc_hash::FxHashMap<
                hir::hir_def::interned::identifier::Ident,
                hir::hir_def::pous::pou::Pou<'db>,
            >,
        >,
    >,
    /// Phase B: call site → mangled specialization (`drive` -> `drive$Worker`),
    /// module-global.
    pub iface_call_rewrites: Option<
        std::rc::Rc<
            rustc_hash::FxHashMap<
                hir::hir_def::expressions::expression::FuncCall<'db>,
                hir::hir_def::interned::identifier::Ident,
            >,
        >,
    >,
    /// Scratch locals synthesized at call sites: `$discard$N` for a discarded
    /// `VAR_OUTPUT`, `$argcopy$N` for an aggregate `VAR_INPUT` snapshot. Drained
    /// into the function's locals by the lowering caller (see `build_call_args`).
    pub call_scratch: std::rc::Rc<std::cell::RefCell<CallScratch>>,
    /// The POU this body is emitted for, which for an inherited method is the
    /// inheritor: `THIS.m()` inside it must reach the inheritor's `m`.
    pub this_pou: Option<hir::hir_def::pous::pou::Pou<'db>>,
}

/// Scratch locals synthesized while lowering calls: `(name, type)`.
#[derive(Debug, Default, Clone)]
pub struct CallScratch {
    /// Memory-forced aggregate snapshots (`$argcopy$`, `$discard$`).
    pub memory: Vec<(hir::hir_def::interned::identifier::Ident, MirType)>,
    /// Scalar wasm-local scratches for extern result destructuring
    /// (`$extret$`, `$extretval$`).
    pub scalar: Vec<(hir::hir_def::interned::identifier::Ident, MirType)>,
}

/// String literal pool: unique strings and their offsets in the data section.
#[derive(Debug, Default)]
pub struct StringPool {
    /// Deduplicated string entries: (offset_in_data_section, bytes).
    pub entries: Vec<(u32, Vec<u8>)>,
    /// Current offset in the data section (grows as strings are added).
    next_offset: u32,
    /// Base offset - string data starts AFTER all static memory (globals, FB instances).
    pub base_offset: u32,
}

impl StringPool {
    pub fn new(base_offset: u32) -> Self {
        Self {
            entries: Vec::new(),
            next_offset: base_offset,
            base_offset,
        }
    }

    /// Intern a literal's decoded bytes (a `$FF` escape is a byte no `&str` can
    /// carry). Returns (id, offset, len).
    pub fn intern(&mut self, bytes: &[u8]) -> (u32, u32, u32) {
        // Check for existing identical string
        for (i, (offset, existing)) in self.entries.iter().enumerate() {
            if existing == bytes {
                return (i as u32, *offset, bytes.len() as u32);
            }
        }
        let id = self.entries.len() as u32;
        let offset = self.next_offset;
        let len = bytes.len() as u32;
        self.entries.push((offset, bytes.to_vec()));
        self.next_offset += len;
        // Align to 4 bytes
        self.next_offset = (self.next_offset + 3) & !3;
        (id, offset, len)
    }

    /// Consume the pool and return the data entries for the WASM data section.
    pub fn into_data(self) -> Vec<(u32, Vec<u8>)> {
        self.entries
    }
}

impl<'db> ExprLowerCtx<'db> {
    pub fn new(db: &'db dyn WorkspaceDataBase, string_pool: Rc<RefCell<StringPool>>) -> Self {
        Self {
            db,
            this_struct: None,
            string_pool,
            iface_subs: None,
            iface_call_rewrites: None,
            call_scratch: Default::default(),
            this_pou: None,
        }
    }

    pub fn with_this_struct(
        db: &'db dyn WorkspaceDataBase,
        struct_type: crate::types::MirStructType,
        string_pool: Rc<RefCell<StringPool>>,
    ) -> Self {
        Self {
            db,
            this_struct: Some(struct_type),
            string_pool,
            iface_subs: None,
            iface_call_rewrites: None,
            call_scratch: Default::default(),
            this_pou: None,
        }
    }

    /// Lower a HIR type to its MIR form (normalizing aliases/variables first).
    pub fn lower_type_resolved(&self, ty: Type<'db>) -> Result<MirType, LowerTypeError> {
        lower_type(self.db, ty.normalize(self.db))
    }

    /// Lower an expression, tagging a failure with the innermost failing
    /// expression's location.
    pub fn lower_expr(&self, expr: Expr<'db>) -> Result<MirExpr, LowerTypeError> {
        use hir::HirNodeInfo;
        self.lower_expr_inner(expr).map_err(|e| {
            e.with_location(
                expr.get_scope_id(self.db).file(self.db),
                expr.get_span(self.db),
            )
        })
    }

    fn lower_expr_inner(&self, expr: Expr<'db>) -> Result<MirExpr, LowerTypeError> {
        match expr.expr(self.db) {
            ExprKind::PrimaryExpr(primary) => self.lower_primary_expr(primary, expr),

            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let op = match operator {
                    AddOperatorKind::Plus => MirBinOp::Add,
                    AddOperatorKind::Minus => MirBinOp::Sub,
                };
                self.lower_binop(op, *left, *right, expr)
            }

            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                let op = match operator {
                    MultOperatorKind::Mul => MirBinOp::Mul,
                    MultOperatorKind::Div => MirBinOp::Div,
                    MultOperatorKind::Mod => MirBinOp::Mod,
                };
                self.lower_binop(op, *left, *right, expr)
            }

            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                let op = match operator {
                    ComparisonOperatorKind::Eq => MirBinOp::Eq,
                    ComparisonOperatorKind::Ne => MirBinOp::Ne,
                    ComparisonOperatorKind::Lt => MirBinOp::Lt,
                    ComparisonOperatorKind::Le => MirBinOp::Le,
                    ComparisonOperatorKind::Gt => MirBinOp::Gt,
                    ComparisonOperatorKind::Ge => MirBinOp::Ge,
                };
                self.lower_comparison(op, *left, *right, expr)
            }

            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                let op = match operator {
                    BooleanOperatorKind::And => MirBinOp::And,
                    BooleanOperatorKind::Or => MirBinOp::Or,
                    BooleanOperatorKind::Xor => MirBinOp::Xor,
                };
                self.lower_binop(op, *left, *right, expr)
            }

            ExprKind::PowerOperator { left, right } => self.lower_power(*left, *right, expr),

            ExprKind::UnaryOperator {
                expr: inner,
                operator,
            } => {
                let op = match operator {
                    UnaryOperatorKind::Plus => {
                        // Plus is a no-op
                        return self.lower_expr(*inner);
                    }
                    UnaryOperatorKind::Minus => MirUnaryOp::Neg,
                    UnaryOperatorKind::Not => MirUnaryOp::Not,
                };
                let inner_mir = self.lower_expr(*inner)?;
                let ty = self.expr_to_mir_elementary(*inner)?;
                Ok(MirExpr::UnaryOp {
                    op,
                    expr: Box::new(inner_mir),
                    ty,
                })
            }

            ExprKind::FoldExpr { .. } => Err(LowerTypeError::UnsupportedType(
                "FoldExpr should be inlined during monomorphization".to_string(),
            )),
        }
    }

    /// Lower a binary operation, inserting explicit casts where needed.
    fn lower_binop(
        &self,
        op: MirBinOp,
        left: Expr<'db>,
        right: Expr<'db>,
        result_expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        let result_elem = self.expr_to_mir_elementary(result_expr)?;

        let left_mir = self.lower_expr_with_cast(left, result_elem)?;
        let right_mir = self.lower_expr_with_cast(right, result_elem)?;

        Ok(MirExpr::BinOp {
            op,
            lhs: Box::new(left_mir),
            rhs: Box::new(right_mir),
            ty: result_elem,
        })
    }

    /// Lower `a ** b`. WASM has no exponentiation instruction, so it lowers to
    /// the grafted `libm` pow as a call, which codegen finds by name.
    fn lower_power(
        &self,
        left: Expr<'db>,
        right: Expr<'db>,
        result_expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        let result_elem = self.expr_to_mir_elementary(result_expr)?;
        // IEC types `**` as `ANY_REAL ** ANY_NUM`, so the result is real and
        // both operands are cast to it — `2.0 ** 3` passes 3 as a float.
        let callee = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(if result_elem.is_64bit() {
                "f64.pow"
            } else {
                "f32.pow"
            }),
        );
        Ok(MirExpr::Call(crate::expr::MirCall {
            callee,
            // Unused: `emit_call` resolves by name through `fn_indices`,
            // where codegen registers the grafted builtin's index.
            callee_index: u32::MAX,
            args: vec![
                crate::expr::MirCallArg {
                    value: self.lower_expr_with_cast(left, result_elem)?,
                    kind: crate::expr::MirArgKind::ByValue,
                },
                crate::expr::MirCallArg {
                    value: self.lower_expr_with_cast(right, result_elem)?,
                    kind: crate::expr::MirArgKind::ByValue,
                },
            ],
            return_type: crate::types::MirType::Elementary(result_elem),
            output_bindings: Vec::new(),
            extern_results: Vec::new(),
            extern_ret_scratch: None,
        }))
    }

    /// Lower a comparison operation. The common type is the wider of the two operand types.
    fn lower_comparison(
        &self,
        op: MirBinOp,
        left: Expr<'db>,
        right: Expr<'db>,
        expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        // STRING operands have no scalar representation — comparison lowers
        // to the grafted `str_byte_cmp` builtin (lexicographic, memcmp-style
        // -1/0/1) and the operator is applied to its result against 0:
        // `a < b`  →  `str_byte_cmp(a, b) < 0`. Detect BEFORE the scalar
        // conversion below, which would otherwise abort the whole build.
        let is_string = |e: Expr<'db>| {
            matches!(
                e.infer(self.db).normalize(self.db),
                hir::hir_ty::ty::Type::Elementary(
                    hir::hir_def::expressions::spec::ElementarySpec::String
                )
            )
        };
        if is_string(left) || is_string(right) {
            return self.lower_string_comparison(op, left, right);
        }

        // The comparison type is inference's decision (`comparison_operand_type`);
        // `wider_type` remains only for enums and subranges, which HIR does not
        // record.
        let common = match hir::hir_ty::body::infer_body(self.db, expr.scope_id(self.db))
            .comparison_operand_type
            .get(&expr)
        {
            Some(ty) => self.type_to_mir_elementary(*ty)?,
            None => {
                let left_elem = self.expr_to_mir_elementary(left)?;
                let right_elem = self.expr_to_mir_elementary(right)?;
                wider_type(left_elem, right_elem)
            }
        };

        let left_mir = self.lower_expr_with_cast(left, common)?;
        let right_mir = self.lower_expr_with_cast(right, common)?;

        Ok(MirExpr::BinOp {
            op,
            lhs: Box::new(left_mir),
            rhs: Box::new(right_mir),
            // Comparison executes at the common type
            ty: common,
        })
    }

    /// Lower a STRING comparison as `str_byte_cmp(a, b) OP 0`; the builtin
    /// takes two by-value STRING args.
    fn lower_string_comparison(
        &self,
        op: MirBinOp,
        left: Expr<'db>,
        right: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        let callee = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from("str.byte_cmp"),
        );
        let cmp = MirExpr::Call(crate::expr::MirCall {
            callee,
            // Unused: `emit_call` resolves by name through `fn_indices`,
            // where codegen registers the grafted builtin's index.
            callee_index: u32::MAX,
            args: vec![
                crate::expr::MirCallArg {
                    value: self.lower_expr(left)?,
                    kind: crate::expr::MirArgKind::ByValue,
                },
                crate::expr::MirCallArg {
                    value: self.lower_expr(right)?,
                    kind: crate::expr::MirArgKind::ByValue,
                },
            ],
            return_type: MirType::Elementary(MirElementary::DInt),
            output_bindings: vec![],
            extern_results: Vec::new(),
            extern_ret_scratch: None,
        });
        Ok(MirExpr::BinOp {
            op,
            lhs: Box::new(cmp),
            rhs: Box::new(MirExpr::Constant(MirConstant::I32(0))),
            ty: MirElementary::DInt,
        })
    }

    /// Lower an expression and insert a cast to the target type if needed.
    fn lower_expr_with_cast(
        &self,
        expr: Expr<'db>,
        target: MirElementary,
    ) -> Result<MirExpr, LowerTypeError> {
        let mir_expr = self.lower_expr(expr)?;
        let expr_elem = self.expr_to_mir_elementary(expr)?;

        if expr_elem == target {
            Ok(mir_expr)
        } else {
            Ok(MirExpr::Cast {
                expr: Box::new(mir_expr),
                from: expr_elem,
                to: target,
            })
        }
    }

    fn lower_primary_expr(
        &self,
        primary: &PrimaryExpr<'db>,
        parent_expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        match primary {
            PrimaryExpr::Literal(elem) => self.lower_literal(elem, parent_expr),

            PrimaryExpr::VariableAccess(var_access) => {
                let place = self.lower_variable_access(*var_access)?;
                // `b.1` reads a slice of `b`, not `b` itself.
                if var_access.multibits(self.db).is_some() {
                    return self.lower_multibit_read(place, *var_access, parent_expr);
                }
                // The Load carries the ADJUSTED type: `arr[1]` produces the element,
                // not the array.
                let ty = self
                    .lower_type_resolved(parent_expr.infer_adjusted(self.db))
                    .unwrap_or(MirType::Void);
                Ok(MirExpr::Load(place, ty))
            }

            PrimaryExpr::FuncCall(func_call) => self.lower_func_call(*func_call, Some(parent_expr)),

            PrimaryExpr::EnumValue { name, variant } => {
                // An enum literal is its variant's ordinal in declaration order, at
                // the enum's storage lane.
                let mut enum_ty = name.infer(self.db).normalize(self.db);
                if !matches!(enum_ty, Type::Enum(_)) {
                    // Initializer paths are typed by init inference, not body inference.
                    let init_res = hir::hir_ty::head::init_inference::infer_initialization(
                        self.db,
                        name.scope_id(self.db),
                    );
                    if let Some(pe) = name.expr(self.db)
                        && let Some(t) = init_res.body_infer_result.type_of_path_expr.get(&pe)
                    {
                        enum_ty = t.normalize(self.db);
                    }
                }
                let Type::Enum(e) = enum_ty else {
                    return Err(LowerTypeError::UnsupportedType(format!(
                        "Enum literal on non-enum type {:?}",
                        enum_ty
                    )));
                };
                let value = super::lower_type::enum_variant_values(self.db, e)?
                    .into_iter()
                    // The variant arrives as written and HIR's resolved one is folded:
                    // reconcile the spellings here.
                    .find(|(v, _)| v.name.ident.caseless(self.db) == variant.ident.caseless(self.db))
                    .map(|(_, value)| value)
                    .ok_or_else(|| {
                        LowerTypeError::UnsupportedType(format!(
                            "Unknown enum variant '{}'",
                            variant.ident.text(self.db)
                        ))
                    })?;
                // The constant's lane is the enum's declared storage (an LINT-based
                // enum is i64).
                let storage = super::lower_type::enum_storage(self.db, e)?;
                Ok(MirExpr::Constant(match storage.size_bytes() {
                    8 => MirConstant::I64(value),
                    _ => MirConstant::I32(value as i32),
                }))
            }

            PrimaryExpr::RefValue { value } => match value {
                RefValue::Null => Ok(MirExpr::Constant(MirConstant::Null)),
                RefValue::Address(path) => {
                    let place = self.lower_begin_path_to_place(*path)?;
                    Ok(MirExpr::AddrOf(place))
                }
            },

            PrimaryExpr::ParenthesizedExpr { expr } => self.lower_expr(*expr),
        }
    }

    fn lower_literal(
        &self,
        elem: &Elementary,
        parent_expr: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        let db = self.db;
        match elem {
            Elementary::Bool(ident) => {
                let text = ident.text(db);
                let val = text.eq_ignore_ascii_case("TRUE") || text.as_str() == "1";
                Ok(MirExpr::Constant(MirConstant::Bool(val)))
            }

            // Signed integers
            Elementary::SInt(int) | Elementary::Int(int) | Elementary::DInt(int) => {
                let val = int.as_i32(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Integer parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::LInt(int) => {
                let val = int.as_i64(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("LInt parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }

            // Unsigned integers parse in the unsigned domain, then reinterpret into
            // the storage lane.
            Elementary::USInt(int) | Elementary::UInt(int) | Elementary::UDInt(int) => {
                let val = int.as_u32(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Unsigned int parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val as i32)))
            }
            Elementary::ULInt(int) => {
                let val = int.as_u64(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("ULInt parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val as i64)))
            }

            // Bit strings: same unsigned-domain rule (`DWORD#3000000000`).
            Elementary::Byte(int) | Elementary::Word(int) | Elementary::DWord(int) => {
                let val = int.as_u32(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Bit string parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val as i32)))
            }
            Elementary::LWord(int) => {
                let val = int.as_u64(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("LWord parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val as i64)))
            }

            // Floats — through the same HIR accessors the checker validated
            // with. A raw `text.parse()` here rejected `1_000.5`: IEC allows
            // underscores, HIR strips them, and the divergence was an ICE on
            // code `rk check` called clean.
            Elementary::Real(ident) => {
                let val = ident.as_f32(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Real parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::F32(val)))
            }
            Elementary::LReal(ident) => {
                let val = ident.as_f64(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("LReal parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::F64(val)))
            }

            // Infer types - resolve using parent expression type. The LEXEME
            // says integer; the resolved TYPE says what the value is. They
            // disagree exactly when an integer literal sits in float context
            // (`3.0 + 2`, `x ** 2`): inference makes the 2 a REAL, and every
            // consumer — including the no-cast-needed check in
            // `lower_expr_with_cast` — believes it. Emitting from the lexeme
            // produced an i32 a float op then consumed: invalid wasm from a
            // program `rk check` called clean.
            Elementary::InferInteger(int) => {
                let ty = parent_expr.infer(db);
                let resolved = match ty.normalize(db) {
                    Type::Elementary(spec) => elementary_spec_to_mir(spec).ok(),
                    _ => None,
                };
                match resolved {
                    Some(e) if e.is_float() => {
                        let val = int.as_i64(db).map_err(|e| {
                            LowerTypeError::UnsupportedType(format!(
                                "InferInteger float error: {}",
                                e
                            ))
                        })?;
                        Ok(MirExpr::Constant(if e.is_64bit() {
                            MirConstant::F64(val as f64)
                        } else {
                            MirConstant::F32(val as f32)
                        }))
                    }
                    // On an unsigned lane the literal is read unsigned and kept as the
                    // same bits.
                    Some(e) if e.is_64bit() => {
                        let val = match e.is_signed() {
                            true => int.as_i64(db).map_err(|e| {
                                LowerTypeError::UnsupportedType(format!(
                                    "InferInteger i64 error: {}",
                                    e
                                ))
                            })?,
                            false => int.as_u64(db).map_err(|e| {
                                LowerTypeError::UnsupportedType(format!(
                                    "InferInteger u64 error: {}",
                                    e
                                ))
                            })? as i64,
                        };
                        Ok(MirExpr::Constant(MirConstant::I64(val)))
                    }
                    _ => {
                        let val = match resolved {
                            Some(e) if !e.is_signed() => int.as_u32(db).map_err(|e| {
                                LowerTypeError::UnsupportedType(format!(
                                    "InferInteger u32 error: {}",
                                    e
                                ))
                            })? as i32,
                            _ => int.as_i32(db).map_err(|e| {
                                LowerTypeError::UnsupportedType(format!(
                                    "InferInteger i32 error: {}",
                                    e
                                ))
                            })?,
                        };
                        Ok(MirExpr::Constant(MirConstant::I32(val)))
                    }
                }
            }
            Elementary::InferFloat(ident) => {
                let ty = parent_expr.infer(db);
                match ty.normalize(db) {
                    Type::Elementary(ElementarySpec::LReal) => {
                        let val = ident.as_f64(db).map_err(|e| {
                            LowerTypeError::UnsupportedType(format!("InferFloat f64 error: {}", e))
                        })?;
                        Ok(MirExpr::Constant(MirConstant::F64(val)))
                    }
                    _ => {
                        let val = ident.as_f32(db).map_err(|e| {
                            LowerTypeError::UnsupportedType(format!("InferFloat f32 error: {}", e))
                        })?;
                        Ok(MirExpr::Constant(MirConstant::F32(val)))
                    }
                }
            }

            // Decode escapes through the HIR helper the checker used, then intern
            // the bytes.
            Elementary::String(ident) => {
                let bytes = ident.as_single_string(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid STRING literal: {e:?}"))
                })?;
                let (id, offset, len) = self.string_pool.borrow_mut().intern(&bytes);
                Ok(MirExpr::StringLiteral { id, offset, len })
            }

            // Char literal (`CHAR#'X'`) — the same decoder; `check` already
            // held it to exactly one byte. CHAR variables are scalar i32
            // (UTF-32) at the WASM ABI; to feed a CHAR into a STRING-shaped
            // slot, the user calls `Std.Convert.CHAR_TO_STRING` explicitly.
            Elementary::Char(ident) => {
                let bytes = ident.as_single_string(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid CHAR literal: {e:?}"))
                })?;
                let codepoint = bytes.first().copied().unwrap_or(0) as u32;
                Ok(MirExpr::Constant(MirConstant::I32(codepoint as i32)))
            }

            // Time and date literals decode to the storage encodings (`types.rs`);
            // the HIR helpers parse and range-check.
            Elementary::Time(ident) => {
                let val = ident.as_time_ms_i32(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid TIME literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::LTime(ident) => {
                let val = ident.as_ltime_ns_i64(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid LTIME literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }
            Elementary::Date(ident) => {
                let val = ident.as_date_days_i32(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid DATE literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::LDate(ident) => {
                let val = ident.as_ldate_days_i64(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid LDATE literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }
            Elementary::TimeOfDay(ident) => {
                let val = ident.as_tod_ms_i32(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid TOD literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::LTod(ident) => {
                let val = ident.as_ltod_ns_i64(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid LTOD literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }
            Elementary::DateAndTime(ident) => {
                let val = ident.as_dt_secs_i32(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid DT literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::LDateTime(ident) => {
                let val = ident.as_ldt_ns_i64(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid LDT literal: {e:?}"))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }
        }
    }

    /// Lower a VariableAccess to a MirPlace.
    pub fn lower_variable_access(
        &self,
        var_access: VariableAccess<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        match var_access.kind(self.db) {
            VariableAccessKind::Symbolic(begin_path) => self.lower_begin_path_to_place(begin_path),
            VariableAccessKind::Direct(_) => Err(LowerTypeError::UnsupportedType(
                "Direct variable access not yet supported".to_string(),
            )),
        }
    }

    /// The base `MirPlace` for a path root. Local, member or global is
    /// [`VariableDecl::storage_class`]'s answer; MIR only looks up where a
    /// member sits in the layout it built.
    ///
    /// [`VariableDecl::storage_class`]: hir::hir_def::pous::variable::VariableDecl::storage_class
    fn root_place(
        &self,
        root: hir::hir_def::expressions::expression::PathExpr<'db>,
        ident: hir::hir_def::interned::identifier::Ident,
    ) -> MirPlace {
        use hir::hir_def::pous::variable::StorageClass;

        // The place carries the name as declared: every downstream table is
        // keyed by declarations.
        if let Some(decl) = self.root_binding(root, ident) {
            let declared = decl.name(self.db);
            match decl.storage_class(self.db) {
                // The address is filled in once the layout is final.
                StorageClass::Global => {
                    return MirPlace::Global {
                        name: Some(declared),
                        address: 0,
                        ty: MirType::Void,
                    };
                }
                // A local shadows a same-named member, as in HIR name resolution.
                StorageClass::Local => return MirPlace::Local(declared),
                StorageClass::InstanceMember => {}
            }
        } else if let Some(root_expr) = root.flatten(self.db).first().map(|s| s.get_expr(self.db))
            && let Some(ty) =
                hir::hir_ty::body::infer_body(self.db, root.scope_id(self.db))
                    .type_of_path_expr
                    .get(&root_expr)
        {
            // The callable's own name is its return slot, held under the declared
            // name.
            use hir::HasName;
            match ty {
                Type::Function(f) => {
                    return MirPlace::Local(f.name(self.db));
                }
                Type::MethodDecl(m) => {
                    return MirPlace::Local(m.get_name_ident(self.db));
                }
                _ => {}
            }
        }

        // No `this` pointer: the root is a local.
        let Some(this_struct) = self.this_struct.as_ref() else {
            return MirPlace::Local(ident);
        };

        match this_struct
            .fields
            .iter()
            .find(|f| f.name.caseless(self.db) == ident.caseless(self.db))
        {
            Some(field) => {
                let this_field = MirPlace::ThisField {
                    field_name: ident,
                    field_offset: field.offset,
                    field_type: field.ty.clone(),
                };
                Self::wrap_inout_deref(field, this_field)
            }
            None => MirPlace::Local(ident),
        }
    }

    /// A `VAR_IN_OUT` member holds a pointer to the caller's l-value, so its
    /// place is wrapped in a `Deref`; a `REF_TO` member is dereferenced only
    /// where the user writes `^`.
    fn wrap_inout_deref(field: &crate::types::MirStructField, place: MirPlace) -> MirPlace {
        match (&field.by_ref, &field.ty) {
            (true, MirType::Pointer(pointee)) => MirPlace::Deref {
                base: Box::new(place),
                pointee_type: (**pointee).clone(),
            },
            _ => place,
        }
    }

    /// The declaration this path's root name binds to.
    ///
    /// Two HIR answers, because neither covers every path on its own. The
    /// body's typed expressions cover a name the enclosing POU does not
    /// declare — direct access to a configuration global — but they are keyed
    /// per expression SHAPE, and the receiver of `g()` roots in an
    /// `Invocation`, which `type_of_path_expr` has no key for. The scope's own
    /// declarations answer that one. (`def_map.global_variables` is every
    /// variable the POU declares, not just the global ones.)
    fn root_binding(
        &self,
        path: hir::hir_def::expressions::expression::PathExpr<'db>,
        ident: hir::hir_def::interned::identifier::Ident,
    ) -> Option<hir::hir_def::pous::variable::VariableDecl<'db>> {
        use hir::hir_ty::body::infer_body;

        let scope = path.scope_id(self.db);
        // `flatten()[0]` is the innermost root step.
        if let Some(root_expr) = path.flatten(self.db).first().map(|s| s.get_expr(self.db))
            && let Some(Type::Variable((var, _))) =
                infer_body(self.db, scope).type_of_path_expr.get(&root_expr)
        {
            return Some(*var);
        }
        scope
            .def_map(self.db)
            .global_variables
            .get(&ident.caseless(self.db))
            .copied()
    }

    /// Lower a BeginPathExpr to a MirPlace, handling nested field/index/deref chains.
    fn lower_begin_path_to_place(
        &self,
        begin_path: hir::hir_def::expressions::expression::BeginPathExpr<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        let path_expr = match begin_path.expr(self.db) {
            Some(pe) => pe,
            None => {
                // Bare `THIS`, as an interface argument: the instance base,
                // `AddrOf(ThisField{0})`.
                if begin_path.invocation(self.db).map(|i| i.kind(self.db))
                    == Some(InvocationKind::This)
                {
                    return Ok(MirPlace::ThisField {
                        field_name: hir::hir_def::interned::identifier::Ident::new(
                            self.db,
                            compact_str::CompactString::from("THIS"),
                        ),
                        field_offset: 0,
                        field_type: MirType::Elementary(MirElementary::Int),
                    });
                }
                return Err(LowerTypeError::UnsupportedType(
                    "Path without path expression".to_string(),
                ));
            }
        };

        // Check for THIS invocation - if present, the path is relative to the 'this' pointer
        if let Some(invocation) = begin_path.invocation(self.db)
            && invocation.kind(self.db) == InvocationKind::This
        {
            return self.lower_this_path(path_expr);
        }

        // Resolve the base variable name from the root VarAccess in the chain
        let base_ident = self.find_root_var_ident(path_expr);

        // Member-vs-local is HIR's decision; `this_struct` only supplies the
        // offset.
        let base = self.root_place(path_expr, base_ident);

        // Walk the path expression chain for field/index/deref
        self.lower_path_expr_chain(base, path_expr)
    }

    /// Lower a path expression rooted at THIS (method instance field access).
    fn lower_this_path(
        &self,
        path_expr: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        match path_expr.expr(self.db) {
            PathExprKind::VarAccess(var) => {
                let field_name = match &var {
                    VarAccess::Simple(span_ident) => span_ident.ident,
                };

                // The layout of the instance this body runs on, as the lowering caller
                // supplied it (the inheritor's, for a copied inherited method).
                let this_type = self.this_struct.clone().map(MirType::Struct);

                // The resolved this-struct field carries the pointer type and `by_ref`
                // flag; the inferred type is the error-recovery fallback.
                if let Some(MirType::Struct(s)) = &this_type
                    && let Some(field) = s
                        .fields
                        .iter()
                        .find(|f| f.name.caseless(self.db) == field_name.caseless(self.db))
                {
                    let this_field = MirPlace::ThisField {
                        field_name,
                        field_offset: field.offset,
                        field_type: field.ty.clone(),
                    };
                    return Ok(Self::wrap_inout_deref(field, this_field));
                }

                let (field_offset, field_type) =
                    self.field_slot_from_mir(&this_type, field_name)?;

                Ok(MirPlace::ThisField {
                    field_name,
                    field_offset,
                    field_type,
                })
            }
            PathExprKind::Field(field_expr) => {
                // Nested field: THIS.a.b - lower the inner path first
                let inner = self.lower_this_path(field_expr.path)?;
                let field_name = match &field_expr.var {
                    VarAccess::Simple(span_ident) => span_ident.ident,
                };
                let base_type = field_expr.path.infer(self.db);
                let (field_offset, field_type) = self.field_slot(base_type, field_name)?;

                Ok(MirPlace::Field {
                    base: Box::new(inner),
                    field_name,
                    field_offset,
                    field_type,
                })
            }
            PathExprKind::Index(index_expr) => {
                let inner = self.lower_this_path(index_expr.path)?;
                self.lower_index_places(inner, &index_expr)
            }
            PathExprKind::Deref(deref_expr) => {
                let inner = self.lower_this_path(deref_expr.path)?;
                let pointee_type = self
                    .lower_type_resolved(path_expr.infer(self.db))
                    .unwrap_or(MirType::Void);
                Ok(MirPlace::Deref {
                    base: Box::new(inner),
                    pointee_type,
                })
            }
        }
    }

    /// Lower `SUPER()` — a call to the immediate base FB's cyclic body on the
    /// current `this`. FBs are single-inheritance (`EXTENDS` at most one base),
    /// so there is exactly one target: `Base$__body__(this)`. HIR already
    /// validated the invocation (E0501/E0513); we resolve the base from the
    /// current FB's `EXTENDS` and pass the current instance pointer
    /// (`AddrOf(ThisField{0})` = `LocalGet(0)`).
    pub fn lower_super_body_call(
        &self,
        // The base is the POU's own EXTENDS; the receiver is the current
        // instance.
        _begin_path: hir::hir_def::expressions::expression::BeginPathExpr<'db>,
    ) -> Result<Option<crate::stmt::MirStmt>, LowerTypeError> {
        use hir::hir_def::pous::pou::Pou;

        // The POU this body belongs to, as the lowering caller named it.
        let current = self.this_pou.ok_or_else(|| {
            LowerTypeError::UnsupportedType("SUPER() outside a function block".to_string())
        })?;
        // The base as HIR resolved it (`base_pou`): MIR does not walk `EXTENDS`.
        let base = hir::hir_ty::head::inheritance::base_pou(self.db, current);
        let base_pou = match base {
            Some(p @ Pou::FunctionBlock(_)) => p,
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "SUPER() base is not a function block".to_string(),
                ));
            }
        };

        // Body function `Base$__body__`, called with the current instance pointer.
        let base_q =
            crate::lower::naming::qualified_pou_ident(self.db, Type::new_pou(self.db, base_pou));
        let body_name = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(format!("{}$__body__", base_q.text(self.db))),
        );
        let this_arg = MirCallArg {
            value: MirExpr::AddrOf(MirPlace::ThisField {
                field_name: hir::hir_def::interned::identifier::Ident::new(
                    self.db,
                    compact_str::CompactString::from("THIS"),
                ),
                field_offset: 0,
                field_type: MirType::Elementary(MirElementary::Int),
            }),
            kind: MirArgKind::ByRef,
        };
        Ok(Some(crate::stmt::MirStmt::Call(MirCall {
            callee: body_name,
            callee_index: 0,
            args: vec![this_arg],
            return_type: MirType::Void,
            output_bindings: Vec::new(),
            extern_results: Vec::new(),
            extern_ret_scratch: None,
        })))
    }

    /// Byte offset of `field_name` within an already-lowered `this` type.
    /// A miss is an error, never offset 0: HIR resolved the access, so a miss
    /// means MIR and HIR disagree.
    fn field_slot_from_mir(
        &self,
        this_type: &Option<MirType>,
        field_name: hir::hir_def::interned::identifier::Ident,
    ) -> Result<(u32, MirType), LowerTypeError> {
        if let Some(MirType::Struct(s)) = this_type {
            for field in &s.fields {
                if field.name.caseless(self.db) == field_name.caseless(self.db) {
                    return Ok((field.offset, field.ty.clone()));
                }
            }
        }
        Err(LowerTypeError::UnsupportedType(format!(
            "field '{}' not found in the enclosing instance layout",
            field_name.text(self.db)
        )))
    }

    /// Recursively lower a path expression chain (field access, indexing, deref).
    fn lower_path_expr_chain(
        &self,
        base: MirPlace,
        path_expr: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        match path_expr.expr(self.db) {
            PathExprKind::Field(field_expr) => {
                let inner = self.lower_path_expr_chain(base, field_expr.path)?;
                let field_name = match &field_expr.var {
                    VarAccess::Simple(span_ident) => span_ident.ident,
                };

                // Offset and type come from the base type's layout in one lookup.
                let base_hir_type = field_expr.path.infer(self.db);
                let (field_offset, field_type) = self.field_slot(base_hir_type, field_name)?;

                Ok(MirPlace::Field {
                    base: Box::new(inner),
                    field_name,
                    field_offset,
                    field_type,
                })
            }

            PathExprKind::Index(index_expr) => {
                let inner = self.lower_path_expr_chain(base, index_expr.path)?;
                self.lower_index_places(inner, &index_expr)
            }

            PathExprKind::Deref(deref_expr) => {
                let inner = self.lower_path_expr_chain(base, deref_expr.path)?;
                let pointee_hir_type = path_expr.infer(self.db);
                let pointee_type = self
                    .lower_type_resolved(pointee_hir_type)
                    .unwrap_or(MirType::Void);
                Ok(MirPlace::Deref {
                    base: Box::new(inner),
                    pointee_type,
                })
            }

            PathExprKind::VarAccess(_) => Ok(base),
        }
    }

    /// The byte offset and type of `field_name` within `base_type`, from one
    /// layout lookup. The type must come from the layout: `Type::normalize`
    /// collapses `STRING[n]`, and the layout kept the capacity.
    fn field_slot(
        &self,
        base_type: Type<'db>,
        field_name: hir::hir_def::interned::identifier::Ident,
    ) -> Result<(u32, MirType), LowerTypeError> {
        let base_mir = self.lower_type_resolved(base_type).ok();
        // If the resolved type is an array, the field access is on the element type
        let effective_mir = match base_mir {
            Some(MirType::Array(a)) => Some(*a.element_type),
            other => other,
        };
        // The name arrives as written; the layout holds it as declared. Folded
        // on both sides.
        if let Some(MirType::Struct(s)) = &effective_mir {
            for field in &s.fields {
                if field.name.caseless(self.db) == field_name.caseless(self.db) {
                    return Ok((field.offset, field.ty.clone()));
                }
            }
        }
        Err(LowerTypeError::UnsupportedType(format!(
            "field '{}' not found in the base type's layout",
            field_name.text(self.db)
        )))
    }

    /// Each chained `Index` addresses one dimension: `m[i]` is dimension 0,
    /// `m[i][j]` dimension 1. Every subscript of one node folds into nested
    /// places, one bounds check and one stride per dimension.
    fn lower_index_places(
        &self,
        mut place: MirPlace,
        index_expr: &hir::hir_def::expressions::expression::IndexExpr<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        if index_expr.index.is_empty() {
            return Err(LowerTypeError::UnsupportedType(
                "Array index without expression".to_string(),
            ));
        }
        let array_hir_type = index_expr.path.infer(self.db);
        let base_dim = self.index_dimension(index_expr.path);
        for (k, sub) in index_expr.index.iter().enumerate() {
            let index = self.lower_expr(*sub)?;
            let (element_type, element_size, lower_bound, dim_size) =
                self.resolve_array_dim_info(array_hir_type, base_dim + k)?;
            let index = self.checked_index(index, lower_bound, dim_size);
            place = MirPlace::Index {
                base: Box::new(place),
                index: Box::new(index),
                element_size,
                element_type,
                lower_bound,
            };
        }
        Ok(place)
    }

    fn index_dimension(&self, path: hir::hir_def::expressions::expression::PathExpr<'db>) -> usize {
        match path.expr(self.db) {
            // A comma group (`a[i, j]`) consumes one dimension per subscript.
            PathExprKind::Index(inner) => inner.index.len() + self.index_dimension(inner.path),
            _ => 0,
        }
    }

    /// `(element_type, byte_stride, lower_bound)` for dimension `dim`,
    /// row-major: `stride = element_size × ∏(later sizes)`.
    fn resolve_array_dim_info(
        &self,
        array_type: Type<'db>,
        dim: usize,
    ) -> Result<(MirType, u32, i64, u32), LowerTypeError> {
        let mir = self.lower_type_resolved(array_type).ok();
        if let Some(MirType::Array(ref a)) = mir {
            let later: u32 = a
                .dimensions
                .get(dim + 1..)
                .unwrap_or(&[])
                .iter()
                .map(|(l, h)| (h - l + 1) as u32)
                .product();
            let stride = a.element_size * later;
            let (lower_bound, dim_size) = a
                .dimensions
                .get(dim)
                .map(|(l, h)| (*l, (h - l + 1).max(0) as u32))
                .unwrap_or((0, 0));
            return Ok((*a.element_type.clone(), stride, lower_bound, dim_size));
        }
        // HIR type-checked the index, so a non-array here is a real disagreement.
        Err(LowerTypeError::UnsupportedType(
            "indexed expression did not resolve to an array type".to_string(),
        ))
    }

    /// Wrap a runtime array subscript in `rk.idx_check`, which raises
    /// "array index out of bounds" through the `$rk_exception` machinery when
    /// the subscript leaves `[lower, lower + size)` - the scan faults with a
    /// message instead of the access computing a NEIGHBOUR'S address, which is
    /// what an unchecked `a[4]` on an `ARRAY[0..2]` used to do, silently, for
    /// as long as the program ran.
    ///
    /// A compile-time in-bounds constant stays bare: the check would be dead
    /// weight, and a literal subscript on a function-block receiver must stay
    /// foldable (`static_fb_base` folds constant indices for dispatch). A
    /// constant OUT of bounds is wrapped like a runtime value - HIR rejects
    /// those it can see, so reaching one here means it slipped through, and
    /// raising beats corrupting.
    fn checked_index(&self, index: MirExpr, lower_bound: i64, dim_size: u32) -> MirExpr {
        if let MirExpr::Constant(MirConstant::I32(k)) = &index {
            let u = (*k as i64).wrapping_sub(lower_bound);
            if u >= 0 && (u as u64) < dim_size as u64 {
                return index;
            }
        }
        let callee = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from("rk.idx_check"),
        );
        MirExpr::Call(crate::expr::MirCall {
            callee,
            // Unused: `emit_call` resolves by name through `fn_indices`,
            // where codegen registers the grafted builtin's index.
            callee_index: u32::MAX,
            args: vec![
                crate::expr::MirCallArg {
                    value: index,
                    kind: crate::expr::MirArgKind::ByValue,
                },
                crate::expr::MirCallArg {
                    value: MirExpr::Constant(MirConstant::I32(lower_bound as i32)),
                    kind: crate::expr::MirArgKind::ByValue,
                },
                crate::expr::MirCallArg {
                    value: MirExpr::Constant(MirConstant::I32(dim_size as i32)),
                    kind: crate::expr::MirArgKind::ByValue,
                },
            ],
            return_type: MirType::Elementary(MirElementary::DInt),
            output_bindings: vec![],
            extern_results: Vec::new(),
            extern_ret_scratch: None,
        })
    }

    /// Wrap `value` in the subrange range-check builtin when `ty` declares
    /// one. The compile-time half is E0802, which rejects the constants it
    /// can see; this is the runtime half, so `s := v` faults with a message
    /// instead of storing a value the type forbids.
    ///
    /// A non-subrange type passes through untouched, so every store site can
    /// call this unconditionally. A bound that did not fold (refused at the
    /// declaration, E0803) skips the check under the Never contract.
    pub(crate) fn checked_range(&self, value: MirExpr, ty: Type<'db>) -> MirExpr {
        let Some(sub) = ty.as_subrange(self.db) else {
            return value;
        };
        let (Some(lower), Some(upper)) =
            hir::hir_ty::infer::const_eval::subrange_bounds(self.db, sub)
        else {
            return value;
        };
        let Ok(base) = self.type_to_mir_elementary_pub(ty) else {
            return value;
        };
        self.checked_range_mir(value, &crate::types::MirSubrangeType { base, lower, upper })
    }

    /// [`Self::checked_range`] with the subrange already lowered, for the
    /// sites that hold a `MirType`. A constant out of range is wrapped like
    /// a runtime value.
    pub(crate) fn checked_range_mir(
        &self,
        value: MirExpr,
        sub: &crate::types::MirSubrangeType,
    ) -> MirExpr {
        let base = sub.base;
        if base.is_float() {
            return value;
        }
        let in_range = |k: i64| {
            if base.is_signed() {
                k >= sub.lower && k <= sub.upper
            } else {
                // Unsigned bases compare as bit patterns: a UDINT bound like
                // 4_000_000_000 is negative as an i64-held i32 constant.
                (k as u64) >= (sub.lower as u64) && (k as u64) <= (sub.upper as u64)
            }
        };
        match &value {
            MirExpr::Constant(MirConstant::I32(k)) if in_range(*k as i64) => return value,
            MirExpr::Constant(MirConstant::I64(k)) if in_range(*k) => return value,
            _ => {}
        }
        let name = match (base.is_64bit(), base.is_signed()) {
            (false, true) => "rk.range_check_i32",
            (false, false) => "rk.range_check_u32",
            (true, true) => "rk.range_check_i64",
            (true, false) => "rk.range_check_u64",
        };
        let bound = |b: i64| {
            if base.is_64bit() {
                MirExpr::Constant(MirConstant::I64(b))
            } else {
                MirExpr::Constant(MirConstant::I32(b as i32))
            }
        };
        let callee = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(name),
        );
        MirExpr::Call(crate::expr::MirCall {
            callee,
            // Unused: `emit_call` resolves by name through `fn_indices`,
            // where codegen registers the grafted builtin's index.
            callee_index: u32::MAX,
            args: vec![
                crate::expr::MirCallArg {
                    value,
                    kind: crate::expr::MirArgKind::ByValue,
                },
                crate::expr::MirCallArg {
                    value: bound(sub.lower),
                    kind: crate::expr::MirArgKind::ByValue,
                },
                crate::expr::MirCallArg {
                    value: bound(sub.upper),
                    kind: crate::expr::MirArgKind::ByValue,
                },
            ],
            return_type: MirType::Elementary(base),
            output_bindings: vec![],
            extern_results: Vec::new(),
            extern_ret_scratch: None,
        })
    }

    /// For an instance-method call `receiver.method(...)`: the mangled callee,
    /// the receiver place (its address is the `this` pointer) and the return
    /// type. `None` for ordinary calls.
    fn resolve_method_call(
        &self,
        path: hir::hir_def::expressions::expression::BeginPathExpr<'db>,
    ) -> Result<
        Option<(
            hir::hir_def::interned::identifier::Ident,
            MirPlace,
            Type<'db>,
        )>,
        LowerTypeError,
    > {
        use hir::HasName;
        use hir::hir_def::expressions::expression::PathExprKind;
        use hir::hir_ty::head::inheritance::MethodRef;
        use hir::hir_ty::ty::CallableType;

        // HIR resolved the callee, walking `EXTENDS` for inherited methods.
        let method = match path.infer(self.db) {
            Type::MethodDecl(m) => m,
            Type::CallableType(CallableType::MethodDecl(m)) => m,
            _ => return Ok(None),
        };

        // `THIS.m()` / `SUPER.m()`: HIR resolved `method` (the override for
        // `THIS`, the base method for `SUPER`, IEC tables 9b/10b). The receiver
        // is the current `this` pointer.
        if let Some(kind) = path.invocation(self.db).map(|i| i.kind(self.db)) {
            match kind {
                InvocationKind::This => {
                    return Ok(Some(self.this_receiver_call(method, "THIS", true)?));
                }
                InvocationKind::Super => {
                    return Ok(Some(self.this_receiver_call(method, "SUPER", false)?));
                }
                // `SUPER()` is a base-body call lowered in `lower_super_body_call`; it
                // never reaches here.
                InvocationKind::SuperBody => return Ok(None),
            }
        }

        // `receiver.method` is a Field whose `.path` is the receiver; a bare
        // `Helper()` is a sibling call with an implicit THIS receiver.
        let field = match path.expr(self.db).map(|pe| pe.expr(self.db)) {
            Some(PathExprKind::Field(fe)) => fe,
            Some(PathExprKind::VarAccess(_)) => {
                return Ok(Some(self.this_receiver_call(method, "bare", true)?));
            }
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "unsupported method-call form (expected `instance.method(...)`)".to_string(),
                ));
            }
        };
        let receiver_path = field.path;
        let root_ident = self.find_root_var_ident(receiver_path);

        let method_decl = match method {
            MethodRef::Declared(md) => md,
            // Phase B: a call through an interface param; the concrete implementer
            // is known via `iface_subs`.
            MethodRef::Prototype(proto) => {
                let concrete = self
                    .iface_subs
                    .as_ref()
                    .and_then(|m| m.get(&root_ident).copied())
                    .ok_or_else(|| {
                        LowerTypeError::UnsupportedType(
                            "interface method call not monomorphized (receiver is not a specialized interface param)"
                                .to_string(),
                        )
                    })?;
                let name = proto.get_name_ident(self.db);
                // Which method implements a prototype is HIR's conformance answer;
                // devirtualizing to it is MIR's.
                match hir::hir_ty::head::inheritance::implementing_method(self.db, concrete, name) {
                    Some(d) => d,
                    None => {
                        return Err(LowerTypeError::UnsupportedType(format!(
                            "no concrete implementation of interface method '{}'",
                            name.text(self.db)
                        )));
                    }
                }
            }
        };

        // `inst.m()` targets the method set of the receiver's static type, so
        // a `Derived` receiver reaches `Derived#m`.
        let callee = match receiver_path.infer(self.db).normalize(self.db) {
            Type::FunctionBlock(fb) => self.method_symbol(
                hir::hir_def::pous::pou::Pou::FunctionBlock(fb),
                method_decl.name(self.db),
            ),
            Type::Class(c) => self.method_symbol(
                hir::hir_def::pous::pou::Pou::Class(c),
                method_decl.name(self.db),
            ),
            // Not an instance type: fall back to where the method was declared.
            _ => self.method_callee_symbol(method_decl)?,
        };

        let receiver = self.lower_receiver_place(receiver_path)?;
        let ret = method
            .return_type(self.db)
            .map(|spec| spec.infer(self.db))
            .unwrap_or(Type::Void);
        Ok(Some((callee, receiver, ret)))
    }

    /// The `#`-mangled symbol of a method: its declaring POU's qualified
    /// name, `#`, the method name. For a base method that is `Base#m`.
    fn method_callee_symbol(
        &self,
        method_decl: hir::hir_def::pous::class::MethodDecl<'db>,
    ) -> Result<hir::hir_def::interned::identifier::Ident, LowerTypeError> {
        use hir::hir_def::scope::ScopeKind;
        use hir::hir_def::semantic_index::get_scope;

        let owner_pou = {
            let method_scope = get_scope(self.db, method_decl.scope_id(self.db));
            let parent = method_scope.parent.ok_or_else(|| {
                LowerTypeError::UnsupportedType("method scope has no owner".to_string())
            })?;
            match get_scope(self.db, parent).kind {
                ScopeKind::Pou(pou) => pou,
                _ => {
                    return Err(LowerTypeError::UnsupportedType(
                        "method owner is not a POU".to_string(),
                    ));
                }
            }
        };

        Ok(self.method_symbol(owner_pou, method_decl.name(self.db)))
    }

    /// `<Owner>#<method>`, where the owner is the POU the body is emitted
    /// for (the inheritor, for an inherited method).
    fn method_symbol(
        &self,
        owner: hir::hir_def::pous::pou::Pou<'db>,
        name: hir::hir_def::interned::identifier::Ident,
    ) -> hir::hir_def::interned::identifier::Ident {
        let owner_mangled =
            crate::lower::naming::qualified_pou_ident(self.db, Type::new_pou(self.db, owner));
        hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(format!(
                "{}#{}",
                owner_mangled.text(self.db),
                name.text(self.db)
            )),
        )
    }

    /// Lower a receiver path (`a`, `a.b`, `arr[i]`) to the place of the FB
    /// instance whose address becomes `this`.
    fn lower_receiver_place(
        &self,
        receiver: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        let root = self.find_root_var_ident(receiver);
        let base = self.root_place(receiver, root);
        self.lower_path_expr_chain(base, receiver)
    }

    /// Lower a function call expression.
    pub fn lower_func_call(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
        call_expr: Option<Expr<'db>>,
    ) -> Result<MirExpr, LowerTypeError> {
        let path = func_call.path(self.db);
        // An instance-method call resolves to the mangled symbol plus a `this`
        // pointer.
        let method_target = self.resolve_method_call(path)?;
        // The namespace-qualified identifier the producer registered in
        // `function_indices`; the bare last segment for callees that do not
        // resolve.
        let callee_name = if let Some(mangled) = self
            .iface_call_rewrites
            .as_ref()
            .and_then(|m| m.get(&func_call).copied())
        {
            // Phase B: an interface arg routes the call to the concrete
            // specialization, overriding the generic method symbol.
            mangled
        } else if let Some((callee, _, _)) = &method_target {
            *callee
        } else if let Some(hir::hir_ty::ty::CallableType::Function(f)) =
            self.resolved_call_of(func_call).map(|r| r.callable)
        {
            // The overload resolution picked.
            crate::lower::naming::mir_function_symbol(self.db, f)
        } else {
            match path.infer(self.db) {
                Type::Function(f) => crate::lower::naming::mir_function_symbol(self.db, f),
                Type::CallableType(hir::hir_ty::ty::CallableType::Function(f)) => {
                    crate::lower::naming::mir_function_symbol(self.db, f)
                }
                _ => path
                    .expr(self.db)
                    .map(|pe| pe.ident(self.db).ident)
                    .ok_or_else(|| {
                        LowerTypeError::UnsupportedType("Function call without name".to_string())
                    })?,
            }
        };

        let mut args = Vec::new();
        let output_bindings = Vec::new();

        let call_params = func_call.params(self.db);

        // The callee's signature, from the `CallableType` HIR stored; the bare
        // variants cover statement calls.
        let callable = match path.infer(self.db) {
            Type::CallableType(ct) => Some(ct),
            Type::Function(f) => Some(hir::hir_ty::ty::CallableType::Function(f)),
            Type::FunctionBlock(fb) => Some(hir::hir_ty::ty::CallableType::FunctionBlock(fb)),
            Type::MethodDecl(m) => Some(hir::hir_ty::ty::CallableType::MethodDecl(m)),
            _ => None,
        };

        let mut extern_results = Vec::new();
        if let Some(callable) = callable {
            self.build_call_args(func_call, callable, &mut args, &mut extern_results)?;
        } else {
            // Unresolved callee: no signature to order against, so call-site order.
            for param in call_params {
                match param.kind(self.db) {
                    ParamAssignKind::NonFormal { value }
                    | ParamAssignKind::FormalInput { value, .. } => {
                        args.push(MirCallArg {
                            value: self.lower_expr(value)?,
                            kind: MirArgKind::ByValue,
                        });
                    }
                    ParamAssignKind::FormalOutput { variable, .. } => {
                        let place = self.lower_variable_access(variable)?;
                        args.push(MirCallArg {
                            value: MirExpr::AddrOf(place),
                            kind: MirArgKind::ByRef,
                        });
                    }
                }
            }
        }

        // A method call prepends the receiver's address as the `this` pointer.
        if let Some((_, receiver, _)) = &method_target {
            args.insert(
                0,
                MirCallArg {
                    value: MirExpr::AddrOf(receiver.clone()),
                    kind: MirArgKind::ByRef,
                },
            );
        }

        // Call-site inference first, then the method's declared return:
        // `path.infer()` on a method is the method type, not its return.
        let return_type = call_expr.map(|e| e.infer(self.db)).unwrap_or_else(|| {
            method_target
                .as_ref()
                .map(|(_, _, ret)| *ret)
                .unwrap_or_else(|| path.infer(self.db))
        });
        let mir_return_type = match return_type.normalize(self.db) {
            Type::Void | Type::Never => MirType::Void,
            ty => self.lower_type_resolved(ty).unwrap_or(MirType::Void),
        };

        // With outputs popping after the call, a declared return value needs
        // somewhere to wait: it is LAST on the stack, so it pops FIRST.
        let extern_ret_scratch = if !extern_results.is_empty() && mir_return_type != MirType::Void {
            let name = hir::hir_def::interned::identifier::Ident::new(
                self.db,
                compact_str::CompactString::from(format!(
                    "$extretval${}",
                    self.call_scratch.borrow().scalar.len()
                )),
            );
            self.call_scratch
                .borrow_mut()
                .scalar
                .push((name, mir_return_type.clone()));
            Some(name)
        } else {
            None
        };

        Ok(MirExpr::Call(MirCall {
            callee: callee_name,
            callee_index: 0, // resolved during module lowering
            args,
            return_type: mir_return_type,
            output_bindings,
            extern_results,
            extern_ret_scratch,
        }))
    }

    /// Build a direct call's argument list in the callee's parameter
    /// declaration order — the order codegen binds them (`emit_call` is purely
    /// positional), which is also the order `lower_func` builds the callee's
    /// signature in.
    ///
    /// Call-site params are matched to declarations through HIR's
    /// `variable_of_param` — the authoritative matching (formal args in any
    /// order, positional args skipping named ones) — instead of re-deriving it
    /// here. An omitted FUNCTION/METHOD input falls back to its declared
    /// constant default; E0233 guarantees one exists when the code
    /// type-checks.
    fn build_call_args(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
        callable: hir::hir_ty::ty::CallableType<'db>,
        args: &mut Vec<MirCallArg>,
        extern_results: &mut Vec<crate::expr::ExternResultBind>,
    ) -> Result<(), LowerTypeError> {
        use hir::hir_def::pous::variable::VariableKind;

        // An extern callee returns its scalar VAR_OUTPUTs on the STACK (in
        // declaration order, before the return value): outputs push no args
        // at all and instead record where each result pops to. E0243 refuses
        // everything an import cannot carry before lowering runs.
        let is_extern = matches!(
            callable,
            hir::hir_ty::ty::CallableType::Function(f)
                if { use hir::HasPragmas; f.extern_pragma(self.db).is_some() }
        );

        // The plan resolution assembled: declared parameters in order, each with
        // its binding. Only the ABI decisions are MIR's.
        let record = self.resolved_call_of(func_call).ok_or_else(|| {
            LowerTypeError::UnsupportedType("call was lowered without a resolved plan".to_string())
        })?;

        // Only FUNCTION/METHOD calls synthesize args for omitted params; an FB
        // call leaves the instance field untouched.
        let fills_defaults = matches!(
            callable,
            hir::hir_ty::ty::CallableType::Function(_)
                | hir::hir_ty::ty::CallableType::MethodDecl(_)
        );

        // Wrap a lowered value as ByRef when the target param is
        // `VAR_IN_OUT` / `VAR_OUTPUT`. Only a Load has an address to take;
        // E0234 refuses everything else upstream, partial accesses included.
        // The old fallback passed the VALUE where the callee expects a
        // POINTER — the callee then dereferenced a bit as an address and
        // wrote to memory near 0, from code `rk check` called clean.
        let to_byref = |mir: MirExpr| -> Result<MirCallArg, LowerTypeError> {
            match mir {
                MirExpr::Load(place, _) => Ok(MirCallArg {
                    value: MirExpr::AddrOf(place),
                    kind: MirArgKind::ByRef,
                }),
                other => Err(LowerTypeError::UnsupportedType(format!(
                    "a by-reference argument needs an address; this one lowered \
                     to {other:?}"
                ))),
            }
        };

        for (var, binding) in &record.params {
            // An interface value is a reference, so an interface-typed param passes
            // the address whatever its kind.
            let by_ref = matches!(
                var.kind(self.db),
                VariableKind::InOut | VariableKind::Output
            ) || crate::lower::mono_iface::is_interface_param(self.db, var);

            match binding {
                hir::hir_ty::body::ParamBinding::Values(values) => {
                    for value in values {
                        let value = *value;
                        let lowered = self.lower_expr(value)?;
                        if by_ref {
                            args.push(to_byref(lowered)?);
                            continue;
                        }
                        // Aggregate VAR_INPUT: copy into a scratch and pass its address (the
                        // call-entry snapshot). Gated on the HIR type: unresolved ANY_* params
                        // are never aggregates.
                        let is_aggregate = matches!(
                            var.spec(self.db).infer(self.db).normalize(self.db),
                            Type::Struct(_) | Type::Array(_)
                        );
                        if fills_defaults && is_aggregate {
                            let var_ty = crate::lower::lower_func::lower_var_type(self.db, *var)?;
                            if matches!(var_ty, MirType::Struct(_) | MirType::Array(_)) {
                                let src = match lowered {
                                    MirExpr::Load(src, _) => MirExpr::AddrOf(src),
                                    // An aggregate-returning call already yields its
                                    // source address.
                                    call @ MirExpr::Call(_) => call,
                                    _ => {
                                        return Err(LowerTypeError::UnsupportedType(
                                            "aggregate VAR_INPUT argument must be a \
                                             variable or a call result"
                                                .to_string(),
                                        ));
                                    }
                                };
                                let name = hir::hir_def::interned::identifier::Ident::new(
                                    self.db,
                                    compact_str::CompactString::from(format!(
                                        "$argcopy${}",
                                        self.call_scratch.borrow().memory.len()
                                    )),
                                );
                                let size = var_ty.size_bytes();
                                self.call_scratch.borrow_mut().memory.push((name, var_ty));
                                args.push(MirCallArg {
                                    value: MirExpr::CopyIntoScratch {
                                        scratch: name,
                                        src: Box::new(src),
                                        size,
                                    },
                                    kind: MirArgKind::ByValue,
                                });
                                continue;
                            }
                        }
                        // A by-value scalar is cast to the PARAM's lane. HIR
                        // accepts an implicitly-widening argument
                        // (`Double(n)` with `n : INT` into `IN : LREAL`), so
                        // without the cast the callee's f64 param received an
                        // i32 — invalid wasm from a program `rk check` called
                        // clean. Non-scalar params (STRING, aggregates,
                        // unresolved ANY_*) have no scalar lane to cast to and
                        // keep the raw value.
                        let value =
                            match self.coercion_lane(value, || var.spec(self.db).infer(self.db)) {
                                Ok(param_elem) => match self.expr_to_mir_elementary(value) {
                                    Ok(arg_elem) if arg_elem != param_elem => MirExpr::Cast {
                                        expr: Box::new(lowered),
                                        from: arg_elem,
                                        to: param_elem,
                                    },
                                    _ => lowered,
                                },
                                Err(_) => lowered,
                            };
                        // A subrange param checks its argument at the door.
                        let value = self.checked_range(value, var.spec(self.db).infer(self.db));
                        args.push(MirCallArg {
                            value,
                            kind: MirArgKind::ByValue,
                        });
                    }
                }
                hir::hir_ty::body::ParamBinding::Output(variable) => {
                    let place = self.lower_variable_access(*variable)?;
                    if is_extern {
                        // The result pops off the stack into a scratch,
                        // then stores to the bound place — no pointer arg.
                        let ty = crate::lower::lower_func::lower_var_type(self.db, *var)?;
                        let scratch = self.extern_result_scratch(ty.clone());
                        extern_results.push(crate::expr::ExternResultBind {
                            scratch,
                            dest: Some(place),
                            ty,
                        });
                    } else {
                        args.push(MirCallArg {
                            value: MirExpr::AddrOf(place),
                            kind: MirArgKind::ByRef,
                        });
                    }
                }
                hir::hir_ty::body::ParamBinding::Default(expr) => {
                    if fills_defaults {
                        args.push(MirCallArg {
                            value: self.lower_expr(*expr)?,
                            kind: MirArgKind::ByValue,
                        });
                    }
                }
                hir::hir_ty::body::ParamBinding::Omitted => {
                    // Zero variadic args is valid; an omitted FB input has
                    // instance storage; a discarded FUNCTION/METHOD output
                    // still needs a pointer arg — synthesized here.
                    if !fills_defaults || var.variadic(self.db) {
                        continue;
                    }
                    match var.kind(self.db) {
                        VariableKind::Output if is_extern => {
                            // A discarded extern output still pops off the stack: a scratch, no
                            // destination.
                            let ty = crate::lower::lower_func::lower_var_type(self.db, *var)?;
                            let scratch = self.extern_result_scratch(ty.clone());
                            extern_results.push(crate::expr::ExternResultBind {
                                scratch,
                                dest: None,
                                ty,
                            });
                        }
                        VariableKind::Output => {
                            // A discarded VAR_OUTPUT still needs a pointer param: point
                            // it at a throwaway memory-forced scratch (a STRING scratch
                            // gets a real buffer).
                            let ty = crate::lower::lower_func::lower_var_type(self.db, *var)?;
                            let name = hir::hir_def::interned::identifier::Ident::new(
                                self.db,
                                compact_str::CompactString::from(format!(
                                    "$discard${}",
                                    self.call_scratch.borrow().memory.len()
                                )),
                            );
                            self.call_scratch.borrow_mut().memory.push((name, ty));
                            args.push(MirCallArg {
                                value: MirExpr::AddrOf(MirPlace::Local(name)),
                                kind: MirArgKind::ByRef,
                            });
                        }
                        VariableKind::Input => {
                            // A required input with nothing bound: HIR reported it; do not shift
                            // the arguments after it.
                            return Err(LowerTypeError::UnsupportedType(format!(
                                "input '{}' was omitted with nothing to pass",
                                var.name(self.db).text(self.db)
                            )));
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// A scalar wasm-local scratch for one extern result (`$extret$N`),
    /// registered on the calling function by the lowering caller.
    fn extern_result_scratch(&self, ty: MirType) -> hir::hir_def::interned::identifier::Ident {
        let name = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(format!(
                "$extret${}",
                self.call_scratch.borrow().scalar.len()
            )),
        );
        self.call_scratch.borrow_mut().scalar.push((name, ty));
        name
    }

    /// Lower an FB invocation statement: write the inputs into the instance,
    /// call `__body__(&instance)`, read the outputs.
    pub fn lower_fb_invocation(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
        fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
    ) -> Result<Option<crate::stmt::MirStmt>, LowerTypeError> {
        let path = func_call.path(self.db);

        // The receiver is a whole path, not a name: `cells[i]()` runs element i
        // and `h.a()` runs h's member a, through the same place pipeline as any
        // access. A bare `THIS()` is still rejected here.
        if path.expr(self.db).is_none() {
            return Err(LowerTypeError::UnsupportedType(
                "FB call without name".to_string(),
            ));
        }
        let instance = self.lower_begin_path_to_place(path)?;

        // Get the FB struct type for field offsets (uses FB subs if available)
        let fb_mir_type = self.lower_type_resolved(hir::hir_ty::ty::Type::FunctionBlock(fb))?;
        let struct_type = match &fb_mir_type {
            MirType::Struct(s) => s,
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "FB type is not a struct".to_string(),
                ));
            }
        };

        // Input writes and output reads from the plan resolution assembled, in
        // declaration order (the pinned evaluation order).
        let mut input_writes = Vec::new();
        let mut output_reads = Vec::new();

        let record = hir::hir_ty::body::infer_body(self.db, path.scope_id(self.db))
            .resolved_calls
            .get(&func_call)
            .cloned()
            .ok_or_else(|| {
                LowerTypeError::UnsupportedType(
                    "FB call was lowered without a resolved plan".to_string(),
                )
            })?;

        for (var, binding) in &record.params {
            let var_name = var.name(self.db);
            match binding {
                hir::hir_ty::body::ParamBinding::Default(_)
                | hir::hir_ty::body::ParamBinding::Omitted => {}
                hir::hir_ty::body::ParamBinding::Values(values) => {
                    for value in values {
                        let value = *value;
                        // HIR matched the param to a declared variable, so the field must exist
                        // in the layout; a miss is HIR and MIR disagreeing.
                        let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name)
                        else {
                            return Err(LowerTypeError::UnsupportedType(format!(
                                "input '{}' has no field in the emitted FB layout",
                                var_name.text(self.db)
                            )));
                        };
                        // VAR_IN_OUT is by-reference: the instance field is
                        // a pointer. Store the address of the caller's l-value ONCE
                        // before the body — the body reads/writes through it, so
                        // there is no value copy-in and (unlike a C-emitting compiler) no copy-out.
                        if field.by_ref {
                            // Must be an l-value (`Load(place, _)`) to take its
                            // address — E0234 rejects everything else upstream,
                            // including partial accesses (`b.%X1`), which lower to
                            // a shifted read. Anything else here used to be
                            // silently SKIPPED: the pointer field kept its stale
                            // value and the body wrote through it.
                            match self.lower_expr(value)? {
                                MirExpr::Load(place, _) => {
                                    input_writes.push((
                                        field.offset,
                                        MirExpr::AddrOf(place),
                                        field.ty.clone(),
                                    ));
                                }
                                other => {
                                    return Err(LowerTypeError::UnsupportedType(format!(
                                        "a VAR_IN_OUT argument needs an address; this one \
                                     lowered to {other:?}"
                                    )));
                                }
                            }
                            continue;
                        }
                        // Plain VAR_INPUT: scalars and STRINGs carry the value, aggregates carry
                        // the source address for a `memory.copy`.
                        let expr = self.lower_expr(value)?;
                        let expr = match &field.ty {
                            MirType::Struct(_) | MirType::Array(_) => match expr {
                                MirExpr::Load(place, _) => MirExpr::AddrOf(place),
                                // An aggregate-returning call yields the source address itself.
                                call @ MirExpr::Call(_) => call,
                                // Not an l-value, not a call: no address to copy from.
                                _ => {
                                    return Err(LowerTypeError::UnsupportedType(
                                        "aggregate VAR_INPUT argument must be a variable \
                                     or a call result"
                                            .to_string(),
                                    ));
                                }
                            },
                            // A scalar input converts to the lane inference accepted, falling back
                            // to the field's own lane.
                            MirType::Elementary(field_elem)
                            | MirType::Subrange(crate::types::MirSubrangeType {
                                base: field_elem,
                                ..
                            }) => {
                                let lane = self.recorded_lane(value).unwrap_or(*field_elem);
                                match self.expr_to_mir_elementary(value) {
                                    Ok(arg_elem) if arg_elem != lane => MirExpr::Cast {
                                        expr: Box::new(expr),
                                        from: arg_elem,
                                        to: lane,
                                    },
                                    _ => expr,
                                }
                            }
                            _ => expr,
                        };
                        // A subrange field checks its input at the door.
                        let expr = match &field.ty {
                            MirType::Subrange(sub) => self.checked_range_mir(expr, sub),
                            _ => expr,
                        };
                        input_writes.push((field.offset, expr, field.ty.clone()));
                    }
                }
                hir::hir_ty::body::ParamBinding::Output(variable) => {
                    // Same contract as inputs: a layout miss is a divergence.
                    let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name) else {
                        return Err(LowerTypeError::UnsupportedType(format!(
                            "output '{}' has no field in the emitted FB layout",
                            var_name.text(self.db)
                        )));
                    };
                    let place = self.lower_variable_access(*variable)?;
                    output_reads.push((field.offset, place, field.ty.clone()));
                }
            }
        }

        // The body function: the FB's qualified name plus `$__body__`.
        let mangled_root = crate::lower::naming::qualified_pou_ident(
            self.db,
            hir::hir_ty::ty::Type::FunctionBlock(fb),
        );
        let body_func = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(format!("{}$__body__", mangled_root.text(self.db))),
        );

        Ok(Some(crate::stmt::MirStmt::FbCall {
            instance,
            body_func,
            body_func_index: 0, // resolved during module lowering
            input_writes,
            output_reads,
        }))
    }

    /// The callee, receiver and return type of a method call whose receiver is
    /// the current instance: `THIS.m()`, `SUPER.m()`, or a bare sibling `m()`.
    fn this_receiver_call(
        &self,
        method: hir::hir_ty::head::inheritance::MethodRef<'db>,
        form: &str,
        virtual_dispatch: bool,
    ) -> Result<
        (
            hir::hir_def::interned::identifier::Ident,
            MirPlace,
            Type<'db>,
        ),
        LowerTypeError,
    > {
        use hir::hir_ty::head::inheritance::MethodRef;
        let method_decl = match method {
            MethodRef::Declared(md) => md,
            MethodRef::Prototype(_) => {
                return Err(LowerTypeError::UnsupportedType(format!(
                    "{form} method call unexpectedly resolved to an interface prototype"
                )));
            }
        };
        // `THIS.m()` and bare `m()` dispatch against the POU this body is emitted
        // for; `SUPER.m()` is static (IEC 9b/10b) and keeps the base.
        let callee = match (virtual_dispatch, self.this_pou) {
            (true, Some(owner)) => self.method_symbol(owner, method_decl.name(self.db)),
            _ => self.method_callee_symbol(method_decl)?,
        };
        let receiver = MirPlace::ThisField {
            field_name: hir::hir_def::interned::identifier::Ident::new(
                self.db,
                compact_str::CompactString::from("THIS"),
            ),
            field_offset: 0,
            field_type: MirType::Elementary(MirElementary::Int),
        };
        let ret = method
            .return_type(self.db)
            .map(|spec| spec.infer(self.db))
            .unwrap_or(Type::Void);
        Ok((callee, receiver, ret))
    }

    fn find_root_var_ident(
        &self,
        path_expr: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> hir::hir_def::interned::identifier::Ident {
        match path_expr.expr(self.db) {
            PathExprKind::VarAccess(var) => match var {
                VarAccess::Simple(span_ident) => span_ident.ident,
            },
            PathExprKind::Field(field_expr) => self.find_root_var_ident(field_expr.path),
            PathExprKind::Index(index_expr) => self.find_root_var_ident(index_expr.path),
            PathExprKind::Deref(deref_expr) => self.find_root_var_ident(deref_expr.path),
        }
    }

    /// Lower a CaseKind to a MirCasePattern.
    pub fn lower_case_kind(
        &self,
        case: &CaseKind<'db>,
        selector: Expr<'db>,
    ) -> Result<MirCasePattern, LowerTypeError> {
        match case {
            // A STRING label compares with `str.byte_cmp`, carried as the arm's own
            // test.
            CaseKind::Expression(expr)
                if matches!(
                    self.expr_type(*expr),
                    Type::Elementary(hir::hir_def::expressions::spec::ElementarySpec::String)
                ) =>
            {
                Ok(MirCasePattern::Test(self.lower_string_comparison(
                    MirBinOp::Eq,
                    selector,
                    *expr,
                )?))
            }
            CaseKind::Expression(expr) => {
                Ok(MirCasePattern::Value(self.case_label_constant(*expr)?))
            }
            CaseKind::Subrange { lower, upper } => Ok(MirCasePattern::Range {
                lower: self.case_label_constant(*lower)?,
                upper: self.case_label_constant(*upper)?,
            }),
        }
    }

    /// The normalized type of an expression, for label classification.
    fn expr_type(&self, expr: Expr<'db>) -> Type<'db> {
        expr.infer(self.db).normalize(self.db)
    }

    /// One CASE label's value, as HIR evaluated it.
    ///
    /// HIR checks a label by EVALUATING it (`case_label_value`), so the answer
    /// already exists and E1006 has refused anything without one. Lowering the
    /// label and inspecting whether a constant fell out re-derived that with a
    /// narrower evaluator, and disagreed: `K:` for a CONSTANT `K` checked clean
    /// and aborted here. An enum label is the exception — its value is its
    /// variant's ordinal, which `lower_expr` reads from the same table the
    /// enum's type does.
    fn case_label_constant(
        &self,
        label: hir::hir_def::expressions::expression::Expr<'db>,
    ) -> Result<MirConstant, LowerTypeError> {
        let body = hir::hir_ty::body::infer_body(self.db, label.scope_id(self.db));
        // Only the integer domain becomes a scalar constant; a string label
        // lowers to its own test.
        if let Some(hir::hir_ty::body::CaseLabelValue::Int(value)) =
            body.case_label_value.get(&label)
        {
            return Ok(match self.expr_to_mir_elementary(label) {
                Ok(elem) if elem.size_bytes() == 8 => MirConstant::I64(*value),
                _ => MirConstant::I32(*value as i32),
            });
        }
        // No recorded value: an enum label.
        expr_to_constant(&self.lower_expr(label)?)
    }

    /// Public accessor for type_to_mir_elementary (used by lower_stmt).
    pub fn type_to_mir_elementary_pub(
        &self,
        ty: Type<'db>,
    ) -> Result<MirElementary, LowerTypeError> {
        self.type_to_mir_elementary(ty)
    }

    /// The plan resolution assembled for this call: from body inference, or
    /// from init inference for a call in an initializer.
    fn resolved_call_of(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
    ) -> Option<hir::hir_ty::body::ResolvedCall<'db>> {
        let scope = func_call.path(self.db).scope_id(self.db);
        hir::hir_ty::body::infer_body(self.db, scope)
            .resolved_calls
            .get(&func_call)
            .or_else(|| {
                hir::hir_ty::head::init_inference::infer_initialization(self.db, scope)
                    .body_infer_result
                    .resolved_calls
                    .get(&func_call)
            })
            .cloned()
    }

    /// The lane inference accepted for this value where it is consumed, when
    /// it recorded one.
    fn recorded_lane(&self, expr: Expr<'db>) -> Option<MirElementary> {
        hir::hir_ty::body::infer_body(self.db, expr.scope_id(self.db))
            .coercion_target
            .get(&expr)
            .copied()
            .and_then(|ty| self.type_to_mir_elementary(ty).ok())
    }

    /// [`Self::recorded_lane`], falling back to a declared type for the shapes
    /// HIR does not record.
    fn coercion_lane(
        &self,
        expr: Expr<'db>,
        fallback: impl FnOnce() -> Type<'db>,
    ) -> Result<MirElementary, LowerTypeError> {
        match self.recorded_lane(expr) {
            Some(lane) => Ok(lane),
            None => self.type_to_mir_elementary(fallback()),
        }
    }

    /// The machine type an expression evaluates to, using the ADJUSTED type:
    /// `arr[0]` is the element, not the array.
    fn expr_to_mir_elementary(&self, expr: Expr<'db>) -> Result<MirElementary, LowerTypeError> {
        let ty = expr.infer_adjusted(self.db);
        self.type_to_mir_elementary(ty)
    }

    fn type_to_mir_elementary(&self, ty: Type<'db>) -> Result<MirElementary, LowerTypeError> {
        let normalized = ty.normalize(self.db);
        match normalized {
            Type::Elementary(spec) => elementary_spec_to_mir(spec),
            Type::Enum(e) => {
                // Enums compare at their declared storage lane.
                super::lower_type::enum_storage(self.db, e)
            }
            // A variant literal is a value of its enum, at the enum's declared
            // storage.
            Type::EnumVariant(dt, _) => self.type_to_mir_elementary(Type::DataType(dt)),
            Type::RefTo(_) | Type::Null => Ok(MirElementary::Int), // pointers are i32
            Type::Void => Err(LowerTypeError::UnsupportedType(
                "expression has no value (used where a single value is expected)".to_string(),
            )),
            // An aggregate has no scalar machine type; indexing yields the element
            // through the adjustments.
            Type::Array(_) => Err(LowerTypeError::UnsupportedType(
                "an ARRAY has no scalar representation (used where a single value is expected)"
                    .to_string(),
            )),
            Type::Struct(_) | Type::StructElement(_) => Err(LowerTypeError::UnsupportedType(
                "a STRUCT has no scalar representation (used where a single value is expected)"
                    .to_string(),
            )),
            // Function/FunctionBlock used as return value - resolve via return type
            Type::Function(f) => {
                if let Some(ret) = f.return_type(self.db) {
                    self.type_to_mir_elementary(ret.infer(self.db))
                } else {
                    Ok(MirElementary::Int)
                }
            }
            Type::CallableType(_ct) => {
                // Already normalized by normalize() but just in case
                self.type_to_mir_elementary(normalized)
            }
            // A literal inference never pinned down: a resolution gap, not a default.
            Type::Infer(_) => Err(LowerTypeError::UnsupportedType(
                "numeric literal type was never resolved".to_string(),
            )),
            _ => Err(LowerTypeError::UnsupportedType(format!(
                "Cannot get elementary type for: {:?}",
                normalized
            ))),
        }
    }
}

/// Extract a constant from a MIR expression (for case patterns).
fn expr_to_constant(expr: &MirExpr) -> Result<MirConstant, LowerTypeError> {
    match expr {
        MirExpr::Constant(c) => Ok(c.clone()),
        _ => Err(LowerTypeError::UnsupportedType(
            "Case pattern must be a constant expression".to_string(),
        )),
    }
}

/// Determine the wider of two elementary types (for implicit promotion).
fn wider_type(a: MirElementary, b: MirElementary) -> MirElementary {
    if a == b {
        return a;
    }

    // Float wins over integer
    if a.is_float() || b.is_float() {
        if a == MirElementary::LReal || b == MirElementary::LReal {
            return MirElementary::LReal;
        }
        return MirElementary::Real;
    }

    // 64-bit wins over 32-bit
    if a.is_64bit() || b.is_64bit() {
        if a.is_signed() || b.is_signed() {
            return MirElementary::LInt;
        }
        return MirElementary::ULInt;
    }

    // Both 32-bit - signed wins
    if a.is_signed() || b.is_signed() {
        return MirElementary::DInt;
    }

    MirElementary::UDInt
}
