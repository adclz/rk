use db::WorkspaceDataBase;
use hir::{
    hir_def::expressions::{
        expression::{
            AddOperatorKind, BooleanOperatorKind, ComparisonOperatorKind, Elementary, Expr,
            ExprKind, InitExprKind, MultOperatorKind, ParamAssignKind, PathExprKind, PrimaryExpr,
            RefValue, UnaryOperatorKind, VarAccess, VariableAccess, VariableAccessKind,
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
    /// Scratch locals synthesized at call sites (`(name, value type)`):
    /// - `$discard$N` — a DISCARDED `VAR_OUTPUT`'s pointer arg points at a
    ///   throwaway local instead of leaving the callee's param unfed;
    /// - `$argcopy$N` — an aggregate `VAR_INPUT` arg is copied into a scratch
    ///   whose address the callee receives (call-entry snapshot, value
    ///   semantics — see `MirExpr::CopyIntoScratch`).
    /// Collected during body lowering; the function-lowering caller drains
    /// them into the `MirFunction`'s locals (memory-forced, so the address
    /// exists). See `build_call_args`.
    pub call_scratch: std::rc::Rc<std::cell::RefCell<CallScratch>>,
}

/// Scratch locals synthesized for call args: `(name, type)`.
pub type CallScratch = Vec<(hir::hir_def::interned::identifier::Ident, MirType)>;

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

    /// Intern a string literal. Returns (id, offset, len).
    /// Deduplicates identical strings.
    pub fn intern(&mut self, text: &str) -> (u32, u32, u32) {
        let bytes = text.as_bytes();
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
        }
    }

    /// Lower a HIR type to its MIR form (normalizing aliases/variables first).
    pub fn lower_type_resolved(&self, ty: Type<'db>) -> Result<MirType, LowerTypeError> {
        lower_type(self.db, ty.normalize(self.db))
    }

    /// Lower a HIR expression to a MIR expression.
    pub fn lower_expr(&self, expr: Expr<'db>) -> Result<MirExpr, LowerTypeError> {
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
                self.lower_comparison(op, *left, *right)
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

            ExprKind::PowerOperator { left, right } => {
                self.lower_binop(MirBinOp::Power, *left, *right, expr)
            }

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

    /// Lower a comparison operation. The common type is the wider of the two operand types.
    fn lower_comparison(
        &self,
        op: MirBinOp,
        left: Expr<'db>,
        right: Expr<'db>,
    ) -> Result<MirExpr, LowerTypeError> {
        // Determine common comparison type (widest of the two)
        let left_elem = self.expr_to_mir_elementary(left)?;
        let right_elem = self.expr_to_mir_elementary(right)?;
        let common = wider_type(left_elem, right_elem);

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
                let ty = self
                    .lower_type_resolved(parent_expr.infer(self.db))
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
                let ordinal = e
                    .variants(self.db)
                    .iter()
                    .position(|v| v.name.ident == variant.ident)
                    .ok_or_else(|| {
                        LowerTypeError::UnsupportedType(format!(
                            "Unknown enum variant '{}'",
                            variant.ident.text(self.db)
                        ))
                    })?;
                Ok(MirExpr::Constant(MirConstant::I32(ordinal as i32)))
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

            // Unsigned integers
            Elementary::USInt(int) | Elementary::UInt(int) | Elementary::UDInt(int) => {
                let val = int.as_i32(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Unsigned int parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::ULInt(int) => {
                let val = int.as_i64(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("ULInt parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }

            // Bit strings
            Elementary::Byte(int) | Elementary::Word(int) | Elementary::DWord(int) => {
                let val = int.as_i32(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Bit string parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I32(val)))
            }
            Elementary::LWord(int) => {
                let val = int.as_i64(db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("LWord parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::I64(val)))
            }

            // Floats
            Elementary::Real(ident) => {
                let text = ident.text(db);
                let val: f32 = text.parse().map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Real parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::F32(val)))
            }
            Elementary::LReal(ident) => {
                let text = ident.text(db);
                let val: f64 = text.parse().map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("LReal parse error: {}", e))
                })?;
                Ok(MirExpr::Constant(MirConstant::F64(val)))
            }

            // Infer types - resolve using parent expression type
            Elementary::InferInteger(int) => {
                let ty = parent_expr.infer(db);
                match ty.normalize(db) {
                    Type::Elementary(spec)
                        if elementary_spec_to_mir(spec).is_ok_and(|e| e.is_64bit()) =>
                    {
                        let val = int.as_i64(db).map_err(|e| {
                            LowerTypeError::UnsupportedType(format!(
                                "InferInteger i64 error: {}",
                                e
                            ))
                        })?;
                        Ok(MirExpr::Constant(MirConstant::I64(val)))
                    }
                    _ => {
                        let val = int.as_i32(db).map_err(|e| {
                            LowerTypeError::UnsupportedType(format!(
                                "InferInteger i32 error: {}",
                                e
                            ))
                        })?;
                        Ok(MirExpr::Constant(MirConstant::I32(val)))
                    }
                }
            }
            Elementary::InferFloat(ident) => {
                let text = ident.text(db);
                let ty = parent_expr.infer(db);
                match ty.normalize(db) {
                    Type::Elementary(ElementarySpec::LReal) => {
                        let val: f64 = text.parse().map_err(|e| {
                            LowerTypeError::UnsupportedType(format!("InferFloat f64 error: {}", e))
                        })?;
                        Ok(MirExpr::Constant(MirConstant::F64(val)))
                    }
                    _ => {
                        let val: f32 = text.parse().map_err(|e| {
                            LowerTypeError::UnsupportedType(format!("InferFloat f32 error: {}", e))
                        })?;
                        Ok(MirExpr::Constant(MirConstant::F32(val)))
                    }
                }
            }

            // String literal — intern UTF-8 bytes in the string pool and
            // emit a `(offset, len)` reference.
            Elementary::String(ident) => {
                let raw = ident.text(self.db).to_string();
                let text = raw.trim_matches('\'').trim_matches('"');
                let (id, offset, len) = self.string_pool.borrow_mut().intern(text);
                Ok(MirExpr::StringLiteral { id, offset, len })
            }

            // Char literal (`CHAR#'X'`) — decode the single character to its
            // UTF-32 code point and emit an `i32` constant. CHAR variables
            // are scalar i32 (UTF-32) at the WASM ABI; to feed a CHAR into
            // a STRING-shaped slot, the user calls `Std.Convert.CHAR_TO_STRING`
            // explicitly.
            Elementary::Char(ident) => {
                let raw = ident.text(self.db).to_string();
                let text = raw.trim_matches('\'').trim_matches('"');
                let codepoint = text.chars().next().map(|c| c as u32).unwrap_or(0);
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

    /// Build the base `MirPlace` for a path root.
    ///
    /// The member-vs-local **decision** is HIR's: MIR must not re-resolve names
    /// against `this_struct`, because that name match ignores shadowing. A
    /// method local that shares a name with an FB member shadows it in
    /// IEC (and in HIR name resolution), so it must lower to a wasm
    /// `Local`, not a `ThisField`. We ask HIR whether the root binds to a
    /// variable declared in the enclosing *method's own scope*; if so it is a
    /// genuine local. For every other case (real members, globals, unresolved
    /// names during error recovery) we fall back to the historical
    /// `this_struct` layout heuristic — `this_struct` is retained ONLY to
    /// resolve a member's this-relative offset, never to decide membership.
    fn root_place(
        &self,
        root: hir::hir_def::expressions::expression::PathExpr<'db>,
        ident: hir::hir_def::interned::identifier::Ident,
    ) -> MirPlace {
        // No `this` pointer (e.g. a free function body): nothing can be a
        // member, so the root is always a local. Also skips the HIR query.
        let Some(this_struct) = self.this_struct.as_ref() else {
            return MirPlace::Local(ident);
        };

        // HIR says this root is one of the method's own locals/params/return →
        // it shadows any same-named member.
        if self.root_is_method_local(root) {
            return MirPlace::Local(ident);
        }

        // Otherwise: legacy heuristic. A name present in the FB/Class layout is
        // a member (offset from `this_struct`); anything else is a local.
        match this_struct.fields.iter().find(|f| f.name == ident) {
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

    /// Did HIR resolve the root of `path` to a variable declared in the current
    /// method/prototype scope (a genuine local/param/return), as opposed to an
    /// FB/Class member, a global, or an unresolved name? Drives `root_place`.
    fn root_is_method_local(
        &self,
        path: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> bool {
        use hir::HirNodeInfo;
        use hir::hir_def::{scope::ScopeKind, semantic_index::get_scope};
        use hir::hir_ty::body::infer_body;

        let body = infer_body(self.db, path.scope_id(self.db));
        // `flatten()[0]` is the innermost root step.
        let Some(root_expr) = path.flatten(self.db).first().map(|s| s.get_expr(self.db)) else {
            return false;
        };
        match body.type_of_path_expr.get(&root_expr).copied() {
            Some(Type::Variable((var, _))) => matches!(
                get_scope(self.db, var.get_scope_id(self.db)).kind,
                ScopeKind::MethodDecl(_) | ScopeKind::MethodProt(_)
            ),
            _ => false,
        }
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

                // Resolve field offset from the this pointer's type
                // The THIS type is resolved from the method's parent FB/Class
                let scope_id = path_expr.scope_id(self.db);
                let this_type = self.resolve_this_type(scope_id);

                // The resolved this-struct field carries the pointer type and `by_ref`
                // flag; the inferred type is the error-recovery fallback.
                if let Some(MirType::Struct(s)) = &this_type
                    && let Some(field) = s.fields.iter().find(|f| f.name == field_name)
                {
                    let this_field = MirPlace::ThisField {
                        field_name,
                        field_offset: field.offset,
                        field_type: field.ty.clone(),
                    };
                    return Ok(Self::wrap_inout_deref(field, this_field));
                }

                let field_type = self
                    .lower_type_resolved(path_expr.infer(self.db))
                    .unwrap_or(MirType::Elementary(MirElementary::Int));
                let field_offset = self.resolve_field_offset_from_mir(&this_type, field_name);

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
                let field_type = self
                    .lower_type_resolved(path_expr.infer(self.db))
                    .unwrap_or(MirType::Void);
                let base_type = field_expr.path.infer(self.db);
                let field_offset = self.resolve_field_offset(base_type, field_name);

                Ok(MirPlace::Field {
                    base: Box::new(inner),
                    field_name,
                    field_offset,
                    field_type,
                })
            }
            PathExprKind::Index(index_expr) => {
                let inner = self.lower_this_path(index_expr.path)?;
                let index = if let Some(first) = index_expr.index.first() {
                    self.lower_expr(*first)?
                } else {
                    return Err(LowerTypeError::UnsupportedType(
                        "Array index without expression".to_string(),
                    ));
                };
                let array_hir_type = index_expr.path.infer(self.db);
                let dim = self.index_dimension(index_expr.path);
                let (element_type, element_size, lower_bound) =
                    self.resolve_array_dim_info(array_hir_type, dim);
                Ok(MirPlace::Index {
                    base: Box::new(inner),
                    index: Box::new(index),
                    element_size,
                    element_type,
                    lower_bound,
                })
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

    /// Resolve the THIS type from the scope (walks up to find the method's parent FB/Class).
    fn resolve_this_type(&self, scope_id: hir::hir_def::scope::ScopeId<'db>) -> Option<MirType> {
        use hir::hir_def::{scope::ScopeKind, semantic_index::get_scope};

        // Walk up the scope chain to find the parent POU (FB or Class)
        let mut current = Some(scope_id);
        while let Some(sid) = current {
            let scope = get_scope(self.db, sid);
            match scope.kind {
                ScopeKind::Pou(hir::hir_def::pous::pou::Pou::FunctionBlock(fb)) => {
                    return self.lower_type_resolved(Type::FunctionBlock(fb)).ok();
                }
                ScopeKind::Pou(hir::hir_def::pous::pou::Pou::Class(class)) => {
                    return self.lower_type_resolved(Type::Class(class)).ok();
                }
                _ => {
                    current = scope.parent;
                }
            }
        }
        None
    }

    /// Like [`resolve_this_type`](Self::resolve_this_type) but returns the
    /// enclosing POU itself (the FB/Class whose instance `THIS` refers to),
    /// walking up from a method scope to its owner. Used to resolve `SUPER()`'s
    /// base from the current FB's `EXTENDS`.
    fn resolve_this_pou(
        &self,
        scope_id: hir::hir_def::scope::ScopeId<'db>,
    ) -> Option<hir::hir_def::pous::pou::Pou<'db>> {
        use hir::hir_def::pous::pou::Pou;
        use hir::hir_def::{scope::ScopeKind, semantic_index::get_scope};

        let mut current = Some(scope_id);
        while let Some(sid) = current {
            let scope = get_scope(self.db, sid);
            match scope.kind {
                ScopeKind::Pou(pou @ (Pou::FunctionBlock(_) | Pou::Class(_))) => return Some(pou),
                _ => current = scope.parent,
            }
        }
        None
    }

    /// Lower `SUPER()` — a call to the immediate base FB's cyclic body on the
    /// current `this`. FBs are single-inheritance (`EXTENDS` at most one base),
    /// so there is exactly one target: `Base$__body__(this)`. HIR already
    /// validated the invocation (E0501/E0513); we resolve the base from the
    /// current FB's `EXTENDS` and pass the current instance pointer
    /// (`AddrOf(ThisField{0})` = `LocalGet(0)`).
    pub fn lower_super_body_call(
        &self,
        begin_path: hir::hir_def::expressions::expression::BeginPathExpr<'db>,
    ) -> Result<Option<crate::stmt::MirStmt>, LowerTypeError> {
        use hir::hir_def::pous::pou::Pou;

        let scope = begin_path.scope_id(self.db);
        let current = self.resolve_this_pou(scope).ok_or_else(|| {
            LowerTypeError::UnsupportedType("SUPER() outside a function block".to_string())
        })?;
        // The base FB from `EXTENDS` (single inheritance -> one target).
        let base = match current {
            Pou::FunctionBlock(fb) => fb
                .extends(self.db)
                .and_then(|spec| spec.infer(self.db).normalize(self.db).as_pou(self.db)),
            _ => None,
        };
        let base_pou = match base {
            Some(p @ Pou::FunctionBlock(_)) => p,
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "SUPER() base is not a function block".to_string(),
                ));
            }
        };

        // Body function `Base$__body__`, called with the current instance pointer.
        let base_q = crate::lower::naming::qualified_pou_ident(
            self.db,
            Type::new_pou(self.db, base_pou),
        );
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
        })))
    }

    fn resolve_field_offset_from_mir(
        &self,
        this_type: &Option<MirType>,
        field_name: hir::hir_def::interned::identifier::Ident,
    ) -> u32 {
        if let Some(MirType::Struct(s)) = this_type {
            for field in &s.fields {
                if field.name == field_name {
                    return field.offset;
                }
            }
        }
        0
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

                // Resolve field type and offset from the base type
                // The PathExpr for the field resolves to the field's type
                let field_hir_type = path_expr.infer(self.db);
                let field_type = self
                    .lower_type_resolved(field_hir_type)
                    .unwrap_or(MirType::Void);

                // Resolve field offset from the base (struct/FB) type
                let base_hir_type = field_expr.path.infer(self.db);
                let field_offset = self.resolve_field_offset(base_hir_type, field_name);

                Ok(MirPlace::Field {
                    base: Box::new(inner),
                    field_name,
                    field_offset,
                    field_type,
                })
            }

            PathExprKind::Index(index_expr) => {
                let inner = self.lower_path_expr_chain(base, index_expr.path)?;
                let index = if let Some(first) = index_expr.index.first() {
                    self.lower_expr(*first)?
                } else {
                    return Err(LowerTypeError::UnsupportedType(
                        "Array index without expression".to_string(),
                    ));
                };

                // Resolve element type from the array's base type
                let array_hir_type = index_expr.path.infer(self.db);
                let dim = self.index_dimension(index_expr.path);
                let (element_type, element_size, lower_bound) =
                    self.resolve_array_dim_info(array_hir_type, dim);

                Ok(MirPlace::Index {
                    base: Box::new(inner),
                    index: Box::new(index),
                    element_size,
                    element_type,
                    lower_bound,
                })
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

    /// Resolve the byte offset of a field within a struct/FB type.
    fn resolve_field_offset(
        &self,
        base_type: Type<'db>,
        field_name: hir::hir_def::interned::identifier::Ident,
    ) -> u32 {
        let base_mir = self.lower_type_resolved(base_type).ok();
        // If the resolved type is an array, the field access is on the element type
        let effective_mir = match base_mir {
            Some(MirType::Array(a)) => Some(*a.element_type),
            other => other,
        };
        if let Some(MirType::Struct(s)) = &effective_mir {
            for field in &s.fields {
                if field.name == field_name {
                    return field.offset;
                }
            }
        }
        0
    }

    /// For a multi-dimensional array, each chained `Index` (`m[i][j]`) addresses
    /// one dimension: the innermost `m[i]` is dimension 0, `m[i][j]` is dimension
    /// 1, etc. The dimension is the number of `Index` nodes below this one in the
    /// path chain. (`path` is the inner path of the current index, so counting
    /// from there yields this index's own dimension.)
    fn index_dimension(&self, path: hir::hir_def::expressions::expression::PathExpr<'db>) -> usize {
        match path.expr(self.db) {
            PathExprKind::Index(inner) => 1 + self.index_dimension(inner.path),
            _ => 0,
        }
    }

    /// `(element_type, byte_stride, lower_bound)` for dimension `dim`,
    /// row-major: `stride = element_size × ∏(later sizes)`.
    fn resolve_array_dim_info(&self, array_type: Type<'db>, dim: usize) -> (MirType, u32, i64) {
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
            let lower_bound = a.dimensions.get(dim).map(|(l, _)| *l).unwrap_or(0);
            return (*a.element_type.clone(), stride, lower_bound);
        }
        (MirType::Void, 4, 0)
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
                InvocationKind::This | InvocationKind::Super => {
                    let method_decl = match method {
                        MethodRef::Declared(md) => md,
                        MethodRef::Prototype(_) => {
                            return Err(LowerTypeError::UnsupportedType(
                                "THIS/SUPER method call unexpectedly resolved to an \
                                 interface prototype"
                                    .to_string(),
                            ));
                        }
                    };
                    let callee = self.method_callee_symbol(method_decl)?;
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
                    return Ok(Some((callee, receiver, ret)));
                }
                // `SUPER()` is a base-body call lowered in `lower_super_body_call`; it
                // never reaches here.
                InvocationKind::SuperBody => return Ok(None),
            }
        }

        // `receiver.method` is a Field whose `.path` is the receiver instance.
        let field = match path.expr(self.db).map(|pe| pe.expr(self.db)) {
            Some(PathExprKind::Field(fe)) => fe,
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
                use hir::HirNodeInfo;
                use hir::hir_def::pous::pou::Pou;
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
                // Prefer the implementer's OWN declared method; fall back to one
                // it inherits from a base. `inherited_methods` on the implementer
                // returns the interface prototype for this name, not the impl.
                let concrete_scope = match concrete {
                    Pou::FunctionBlock(fb) => fb.get_scope_id(self.db),
                    Pou::Class(c) => c.get_scope_id(self.db),
                    _ => {
                        return Err(LowerTypeError::UnsupportedType(
                            "interface implementer is not a function block or class".to_string(),
                        ));
                    }
                };
                let resolved = concrete_scope
                    .def_map(self.db)
                    .declared_methods
                    .get(&name)
                    .copied()
                    .or_else(|| {
                        hir::hir_ty::head::inheritance::inherited_methods(self.db, concrete)
                            .methods
                            .get(&name)
                            .map(|im| im.method)
                    });
                match resolved {
                    Some(MethodRef::Declared(d)) => d,
                    _ => {
                        return Err(LowerTypeError::UnsupportedType(format!(
                            "no concrete implementation of interface method '{}'",
                            name.text(self.db)
                        )));
                    }
                }
            }
        };

        let callee = self.method_callee_symbol(method_decl)?;

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

        let owner_mangled = crate::lower::naming::qualified_pou_ident(
            self.db,
            Type::new_pou(self.db, owner_pou),
        );
        Ok(hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(format!(
                "{}#{}",
                owner_mangled.text(self.db),
                method_decl.name(self.db).text(self.db)
            )),
        ))
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
        let callee_name = if let Some((callee, _, _)) = &method_target {
            *callee
        } else if let Some(mangled) = self
            .iface_call_rewrites
            .as_ref()
            .and_then(|m| m.get(&func_call).copied())
        {
            // Phase B: this call passes an interface arg to a function with an
            // interface param — route it to the concrete specialization
            // (`drive` -> `drive$Worker`).
            mangled
        } else {
            match path.infer(self.db) {
                Type::Function(f) => {
                    crate::lower::naming::mir_function_symbol(self.db, f)
                }
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

        if let Some(callable) = callable {
            self.build_call_args(func_call, callable, &mut args)?;
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

        Ok(MirExpr::Call(MirCall {
            callee: callee_name,
            callee_index: 0, // resolved during module lowering
            args,
            return_type: mir_return_type,
            output_bindings,
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
    ) -> Result<(), LowerTypeError> {
        use hir::hir_def::pous::variable::VariableKind;

        let path = func_call.path(self.db);
        let body = hir::hir_ty::body::infer_body(self.db, path.scope_id(self.db));

        // Declared param → the call-site assigns bound to it, in call order
        // (a variadic param collects several).
        let mut assigns_of_var: rustc_hash::FxHashMap<
            hir::hir_def::pous::variable::VariableDecl<'db>,
            Vec<hir::hir_def::expressions::expression::ParamAssign<'db>>,
        > = rustc_hash::FxHashMap::default();
        for pa in func_call.params(self.db) {
            if let Some(var) = body.variable_of_param.get(pa) {
                assigns_of_var.entry(*var).or_default().push(*pa);
            }
        }

        // Only FUNCTION/METHOD calls synthesize args for omitted params; an FB
        // call leaves the instance field untouched.
        let fills_defaults = matches!(
            callable,
            hir::hir_ty::ty::CallableType::Function(_)
                | hir::hir_ty::ty::CallableType::MethodDecl(_)
        );

        // Wrap a lowered value as ByRef when the target param is
        // `VAR_IN_OUT` / `VAR_OUTPUT`. Falls back to ByValue when the value
        // isn't a simple Load (we still need to lower function-style
        // expressions where we have no place to take an address of -
        // the type checker rejects those before they get here).
        let to_byref = |mir: MirExpr| match mir {
            MirExpr::Load(place, _) => MirCallArg {
                value: MirExpr::AddrOf(place),
                kind: MirArgKind::ByRef,
            },
            other => MirCallArg {
                value: other,
                kind: MirArgKind::ByValue,
            },
        };

        for var in callable.def_map(self.db).local_variables.values() {
            // An interface value is a reference, so an interface-typed param passes
            // the address whatever its kind.
            let by_ref = matches!(
                var.kind(self.db),
                VariableKind::InOut | VariableKind::Output
            ) || crate::lower::mono_iface::is_interface_param(self.db, var);

            let Some(assigns) = assigns_of_var.get(var) else {
                // Omitted at the call site. Zero variadic args is valid; an
                // omitted FB input has instance storage; FUNCTION/METHOD
                // inputs fall back to their constant default (E0233 rejects
                // the rest); a discarded FUNCTION/METHOD output still needs a
                // pointer arg — synthesized below. Aggregate defaults stay
                // unsupported.
                if fills_defaults && !var.variadic(self.db) {
                    match var.kind(self.db) {
                        VariableKind::Input => {
                            if let Some(init) = var.init(self.db)
                                && let InitExprKind::ConstantExpr(expr) = init.kind(self.db)
                            {
                                args.push(MirCallArg {
                                    value: self.lower_expr(expr)?,
                                    kind: MirArgKind::ByValue,
                                });
                            }
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
                                    self.call_scratch.borrow().len()
                                )),
                            );
                            self.call_scratch.borrow_mut().push((name, ty));
                            args.push(MirCallArg {
                                value: MirExpr::AddrOf(MirPlace::Local(name)),
                                kind: MirArgKind::ByRef,
                            });
                        }
                        _ => {}
                    }
                }
                continue;
            };

            for pa in assigns {
                match pa.kind(self.db) {
                    ParamAssignKind::NonFormal { value }
                    | ParamAssignKind::FormalInput { value, .. } => {
                        let lowered = self.lower_expr(value)?;
                        if by_ref {
                            args.push(to_byref(lowered));
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
                                let MirExpr::Load(src, _) = lowered else {
                                    return Err(LowerTypeError::UnsupportedType(
                                        "aggregate VAR_INPUT argument must be a variable"
                                            .to_string(),
                                    ));
                                };
                                let name = hir::hir_def::interned::identifier::Ident::new(
                                    self.db,
                                    compact_str::CompactString::from(format!(
                                        "$argcopy${}",
                                        self.call_scratch.borrow().len()
                                    )),
                                );
                                let size = var_ty.size_bytes();
                                self.call_scratch.borrow_mut().push((name, var_ty));
                                args.push(MirCallArg {
                                    value: MirExpr::CopyIntoScratch {
                                        scratch: name,
                                        src,
                                        size,
                                    },
                                    kind: MirArgKind::ByValue,
                                });
                                continue;
                            }
                        }
                        args.push(MirCallArg {
                            value: lowered,
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
        Ok(())
    }

    /// Lower an FB invocation statement: write the inputs into the instance,
    /// call `__body__(&instance)`, read the outputs.
    pub fn lower_fb_invocation(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
        fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
    ) -> Result<Option<crate::stmt::MirStmt>, LowerTypeError> {
        let path = func_call.path(self.db);

        // Get the instance variable path (the callee is a variable, not a type)
        let instance_path = path
            .expr(self.db)
            .ok_or_else(|| LowerTypeError::UnsupportedType("FB call without name".to_string()))?;
        let instance_ident = instance_path.ident(self.db).ident;

        // Member-vs-local is HIR's decision (see `root_place`): a nested FB
        // instance that is a 'this' member lowers to ThisField, but a same-named
        // method local shadows it.
        let instance = self.root_place(instance_path, instance_ident);

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

        // Build input writes from the call arguments. Each call-site param is
        // matched to its declared variable through HIR's `variable_of_param` —
        // the authoritative matching, which handles positional args mixed with
        // named ones (a positional arg binds to the first input not claimed by
        // name, regardless of call order).
        let mut input_writes = Vec::new();
        let mut output_reads = Vec::new();

        let body = hir::hir_ty::body::infer_body(self.db, path.scope_id(self.db));

        for param in func_call.params(self.db) {
            let Some(var) = body.variable_of_param.get(param) else {
                continue; // unmatched param - error already reported by HIR
            };
            let var_name = var.name(self.db);
            match param.kind(self.db) {
                ParamAssignKind::FormalInput { value, .. }
                | ParamAssignKind::NonFormal { value } => {
                    let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name) else {
                        continue;
                    };
                    // VAR_IN_OUT is by-reference: the instance field is
                    // a pointer. Store the address of the caller's l-value ONCE
                    // before the body — the body reads/writes through it, so
                    // there is no value copy-in and (unlike a C-emitting compiler) no copy-out.
                    if field.by_ref {
                        // Must be an l-value (`Load(place, _)`) to take its
                        // address — E0234 rejects everything else upstream.
                        if let MirExpr::Load(place, _) = self.lower_expr(value)? {
                            input_writes.push((
                                field.offset,
                                MirExpr::AddrOf(place),
                                field.ty.clone(),
                            ));
                        }
                        continue;
                    }
                    // Plain VAR_INPUT: scalars and STRINGs carry the value, aggregates carry
                    // the source address for a `memory.copy`.
                    let expr = self.lower_expr(value)?;
                    let expr = match &field.ty {
                        MirType::Struct(_) | MirType::Array(_) => match expr {
                            MirExpr::Load(place, _) => MirExpr::AddrOf(place),
                            // No address to copy from (not an l-value) —
                            // nothing sensible to store.
                            _ => continue,
                        },
                        _ => expr,
                    };
                    input_writes.push((field.offset, expr, field.ty.clone()));
                }
                ParamAssignKind::FormalOutput { variable, .. } => {
                    if let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name) {
                        let place = self.lower_variable_access(variable)?;
                        output_reads.push((field.offset, place, field.ty.clone()));
                    }
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
    pub fn lower_case_kind(&self, case: &CaseKind<'db>) -> Result<MirCasePattern, LowerTypeError> {
        match case {
            CaseKind::Expression(expr) => {
                let mir_expr = self.lower_expr(*expr)?;
                let constant = expr_to_constant(&mir_expr)?;
                Ok(MirCasePattern::Value(constant))
            }
            CaseKind::Subrange { lower, upper } => {
                let lower_mir = self.lower_expr(*lower)?;
                let upper_mir = self.lower_expr(*upper)?;
                Ok(MirCasePattern::Range {
                    lower: expr_to_constant(&lower_mir)?,
                    upper: expr_to_constant(&upper_mir)?,
                })
            }
        }
    }

    /// Public accessor for type_to_mir_elementary (used by lower_stmt).
    pub fn type_to_mir_elementary_pub(
        &self,
        ty: Type<'db>,
    ) -> Result<MirElementary, LowerTypeError> {
        self.type_to_mir_elementary(ty)
    }

    /// Get the MirElementary type of an expression.
    fn expr_to_mir_elementary(&self, expr: Expr<'db>) -> Result<MirElementary, LowerTypeError> {
        let ty = expr.infer(self.db);
        self.type_to_mir_elementary(ty)
    }

    fn type_to_mir_elementary(&self, ty: Type<'db>) -> Result<MirElementary, LowerTypeError> {
        let normalized = ty.normalize(self.db);
        match normalized {
            Type::Elementary(spec) => elementary_spec_to_mir(spec),
            Type::Enum(_) => {
                // Enums compare as their storage type
                Ok(MirElementary::DInt)
            }
            // A variant literal (`Color#Green`) is a value of its enum, which
            // stores as DInt (see `lower_enum_type`).
            Type::EnumVariant(_) => Ok(MirElementary::DInt),
            Type::SubRange(sr) => {
                let base = sr._type(self.db).infer(self.db);
                self.type_to_mir_elementary(base)
            }
            Type::RefTo(_) | Type::Null => Ok(MirElementary::Int), // pointers are i32
            Type::Void => Ok(MirElementary::Int),
            // Array indexing: the HIR stores the array type in type_of_path_expr,
            // but the actual expression type after indexing is the element type.
            Type::Array(arr) => {
                let elem_type = arr.of_type(self.db).infer(self.db);
                self.type_to_mir_elementary(elem_type)
            }
            Type::Struct(_) | Type::StructElement(_) => {
                Ok(MirElementary::Int) // fallback
            }
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
            Type::Infer(_infer_ty) => {
                // Deferred integer/float - default to i32/f32
                Ok(MirElementary::Int)
            }
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
