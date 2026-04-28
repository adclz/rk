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
    /// When monomorphizing an ANY_* function, this holds the concrete
    /// ElementarySpec to substitute for ANY types.
    pub any_override: Option<ElementarySpec>,
    /// The `this` struct when lowering an FB body: member accesses become
    /// `ThisField`.
    pub this_struct: Option<crate::types::MirStructType>,
    /// FB ANY_* substitutions - maps FB name → (var name → concrete ElementarySpec).
    pub fb_subs: Option<
        std::rc::Rc<
            rustc_hash::FxHashMap<
                hir::hir_def::interned::identifier::Ident,
                rustc_hash::FxHashMap<hir::hir_def::interned::identifier::Ident, ElementarySpec>,
            >,
        >,
    >,
    /// Per-variable mangled FB-instance name lookup. Built per-function
    /// from the module's `FbInstanceMap`: each entry maps a local
    /// variable's name to the mangled FB name of its concrete
    /// instantiation (e.g. `c_int` → `Counter$INT`). Used when
    /// constructing `FbCall.body_func` so call sites land on the
    /// correct per-T `__body__`.
    pub local_fb_mangling: Option<
        std::rc::Rc<
            rustc_hash::FxHashMap<
                hir::hir_def::interned::identifier::Ident,
                hir::hir_def::interned::identifier::Ident,
            >,
        >,
    >,
    /// String literal pool - shared across all functions in the module.
    pub string_pool: std::rc::Rc<std::cell::RefCell<StringPool>>,
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
            any_override: None,
            this_struct: None,
            fb_subs: None,
            local_fb_mangling: None,
            string_pool,
        }
    }

    pub fn with_any_override(
        db: &'db dyn WorkspaceDataBase,
        concrete: ElementarySpec,
        string_pool: Rc<RefCell<StringPool>>,
    ) -> Self {
        Self {
            db,
            any_override: Some(concrete),
            this_struct: None,
            fb_subs: None,
            local_fb_mangling: None,
            string_pool,
        }
    }

    pub fn with_this_struct(
        db: &'db dyn WorkspaceDataBase,
        struct_type: crate::types::MirStructType,
        string_pool: Rc<RefCell<StringPool>>,
    ) -> Self {
        Self {
            db,
            any_override: None,
            this_struct: Some(struct_type),
            fb_subs: None,
            local_fb_mangling: None,
            string_pool,
        }
    }

    /// Lower a HIR type, substituting ANY types if we're in a monomorphization context.
    pub fn lower_type_resolved(&self, ty: Type<'db>) -> Result<MirType, LowerTypeError> {
        let normalized = ty.normalize(self.db);
        match (&normalized, self.any_override) {
            (Type::Elementary(e), Some(concrete)) if e.is_any() => {
                let mir = elementary_spec_to_mir(concrete)?;
                Ok(MirType::Elementary(mir))
            }
            (Type::Elementary(e), None) if e.is_any() => {
                if let Some(concrete) = self.resolve_any_from_fb_subs(*e) {
                    let mir = elementary_spec_to_mir(concrete)?;
                    Ok(MirType::Elementary(mir))
                } else {
                    lower_type(self.db, normalized)
                }
            }
            (Type::FunctionBlock(fb), _) => {
                // Use FB substitutions if available
                if let Some(ref subs_map) = self.fb_subs
                    && let Some(subs) = subs_map.get(&fb.name(self.db))
                {
                    return crate::lower::lower_type::lower_fb_type_with_subs(self.db, *fb, subs);
                }
                lower_type(self.db, normalized)
            }
            _ => lower_type(self.db, normalized),
        }
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

            PrimaryExpr::EnumValue {
                name: _,
                variant: _,
            } => {
                // Enum values are integer constants - resolve via type inference
                let ty = parent_expr.infer(self.db);
                match ty.normalize(self.db) {
                    Type::EnumVariant(_) | Type::Elementary(_) => {
                        // For now, emit as i32 constant based on variant index
                        // TODO: resolve actual enum variant value
                        Ok(MirExpr::Constant(MirConstant::I32(0)))
                    }
                    _ => Err(LowerTypeError::UnsupportedType(format!(
                        "Enum value with type {:?}",
                        ty
                    ))),
                }
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

            // String/Char literals - intern in the string pool
            Elementary::String(ident)
            | Elementary::WString(ident)
            | Elementary::Char(ident)
            | Elementary::WChar(ident) => {
                let raw = ident.text(self.db).to_string();
                // Strip surrounding quotes (' or ")
                let text = raw.trim_matches('\'').trim_matches('"');
                let (id, offset, len) = self.string_pool.borrow_mut().intern(text);
                Ok(MirExpr::StringLiteral { id, offset, len })
            }

            // Time literals - stored as nanoseconds (i64)
            Elementary::Time(ident) | Elementary::LTime(ident) => {
                let duration = ident.as_time(self.db).map_err(|e| {
                    LowerTypeError::UnsupportedType(format!("Invalid time literal: {:?}", e))
                })?;
                let nanos = duration.whole_nanoseconds() as i64;
                Ok(MirExpr::Constant(MirConstant::I64(nanos)))
            }

            // Date/DateTime/TimeOfDay - not yet supported
            Elementary::Date(_)
            | Elementary::LDate(_)
            | Elementary::DateAndTime(_)
            | Elementary::LDateTime(_)
            | Elementary::TimeOfDay(_)
            | Elementary::LTod(_) => Err(LowerTypeError::UnsupportedType(
                "Date/DateTime literals not yet supported".to_string(),
            )),
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

    /// Lower a BeginPathExpr to a MirPlace, handling nested field/index/deref chains.
    fn lower_begin_path_to_place(
        &self,
        begin_path: hir::hir_def::expressions::expression::BeginPathExpr<'db>,
    ) -> Result<MirPlace, LowerTypeError> {
        let path_expr = begin_path.expr(self.db).ok_or_else(|| {
            LowerTypeError::UnsupportedType("Path without path expression".to_string())
        })?;

        // Check for THIS invocation - if present, the path is relative to the 'this' pointer
        if let Some(invocation) = begin_path.invocation(self.db)
            && invocation.kind(self.db) == InvocationKind::This
        {
            return self.lower_this_path(path_expr);
        }

        // Resolve the base variable name from the root VarAccess in the chain
        let base_ident = self.find_root_var_ident(path_expr);

        // In FB body context, check if this variable is a field of the 'this' struct
        if let Some(ref this_struct) = self.this_struct
            && let Some(field) = this_struct.fields.iter().find(|f| f.name == base_ident)
        {
            let base = MirPlace::ThisField {
                field_name: base_ident,
                field_offset: field.offset,
                field_type: field.ty.clone(),
            };
            return self.lower_path_expr_chain(base, path_expr);
        }
        let base = MirPlace::Local(base_ident);

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
                let field_type = self
                    .lower_type_resolved(path_expr.infer(self.db))
                    .unwrap_or(MirType::Elementary(MirElementary::Int));

                // Resolve field offset from the this pointer's type
                // The THIS type is resolved from the method's parent FB/Class
                let scope_id = path_expr.scope_id(self.db);
                let this_type = self.resolve_this_type(scope_id);
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
                let (element_type, element_size, lower_bound) =
                    self.resolve_array_info(array_hir_type);
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
                let (element_type, element_size, lower_bound) =
                    self.resolve_array_info(array_hir_type);

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

    /// Resolve array element info from an array type.
    fn resolve_array_info(&self, array_type: Type<'db>) -> (MirType, u32, i64) {
        let mir = self.lower_type_resolved(array_type).ok();
        if let Some(MirType::Array(ref a)) = mir {
            let lower_bound = a.dimensions.first().map(|(l, _)| *l).unwrap_or(0);
            return (*a.element_type.clone(), a.element_size, lower_bound);
        }
        (MirType::Void, 4, 0)
    }

    /// Lower a function call expression.
    pub fn lower_func_call(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
        call_expr: Option<Expr<'db>>,
    ) -> Result<MirExpr, LowerTypeError> {
        let path = func_call.path(self.db);
        // The namespace-qualified identifier the producer registered in
        // `function_indices`; the bare last segment for callees that do not
        // resolve.
        let callee_name = match path.infer(self.db) {
            Type::Function(f) => crate::lower::monomorphize::qualified_pou_ident(
                self.db,
                Type::Function(f),
            ),
            Type::CallableType(hir::hir_ty::ty::CallableType::Function(f)) => {
                crate::lower::monomorphize::qualified_pou_ident(self.db, Type::Function(f))
            }
            _ => path
                .expr(self.db)
                .map(|pe| pe.ident(self.db).ident)
                .ok_or_else(|| {
                    LowerTypeError::UnsupportedType("Function call without name".to_string())
                })?,
        };

        let mut args = Vec::new();
        let output_bindings = Vec::new();

        // Track which params were explicitly provided (by position for non-formal, by name for formal)
        let call_params = func_call.params(self.db);
        let mut provided_names: rustc_hash::FxHashSet<hir::hir_def::interned::identifier::Ident> =
            rustc_hash::FxHashSet::default();

        for param in call_params.iter() {
            match param.kind(self.db) {
                ParamAssignKind::NonFormal { value } => {
                    args.push(MirCallArg {
                        value: self.lower_expr(value)?,
                        kind: MirArgKind::ByValue,
                    });
                }
                ParamAssignKind::FormalInput {
                    value,
                    param: param_ident,
                } => {
                    provided_names.insert(param_ident.ident);
                    args.push(MirCallArg {
                        value: self.lower_expr(value)?,
                        kind: MirArgKind::ByValue,
                    });
                }
                ParamAssignKind::FormalOutput {
                    variable,
                    param: param_ident,
                    ..
                } => {
                    provided_names.insert(param_ident.ident);
                    let place = self.lower_variable_access(variable)?;
                    args.push(MirCallArg {
                        value: MirExpr::AddrOf(place),
                        kind: MirArgKind::ByRef,
                    });
                }
            }
        }

        // Fill in default values for omitted parameters.
        // Resolve the callee's variable declarations to find params with defaults.
        // Use the raw inferred type (before normalization) to get the CallableType.
        let callee_type_raw = path.infer(self.db);
        let callable = match callee_type_raw {
            Type::CallableType(ct) => Some(ct),
            Type::Function(f) => Some(hir::hir_ty::ty::CallableType::Function(f)),
            Type::FunctionBlock(fb) => Some(hir::hir_ty::ty::CallableType::FunctionBlock(fb)),
            _ => None,
        };
        if let Some(callable) = callable {
            let def_map = callable.def_map(self.db);
            let provided_count = call_params.len();

            for (i, (var_name, var_decl)) in def_map.local_variables.iter().enumerate() {
                // Skip params that were explicitly provided
                if i < provided_count && provided_names.is_empty() {
                    // Non-formal call: first N params are positional
                    continue;
                }
                if provided_names.contains(var_name) {
                    continue;
                }
                // Only fill defaults for params beyond what was provided positionally
                if i < provided_count {
                    continue;
                }

                // Check if this variable has a default value
                if let Some(init) = &var_decl.init(self.db) {
                    match init.kind(self.db) {
                        InitExprKind::ConstantExpr(expr) => {
                            match self.lower_expr(expr) {
                                Ok(mir_expr) => {
                                    args.push(MirCallArg {
                                        value: mir_expr,
                                        kind: MirArgKind::ByValue,
                                    });
                                }
                                Err(_) => {
                                    // Failed to lower default - skip
                                }
                            }
                        }
                        _ => {
                            // Complex init (struct/array) - not yet supported as default
                        }
                    }
                }
            }
        }

        // Return type: use call-site inference (resolves ANY → concrete) when available,
        // fallback to path inference for statement-level calls (void return).
        let return_type = call_expr
            .map(|e| e.infer(self.db))
            .unwrap_or_else(|| path.infer(self.db));
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

    /// Lower an FB invocation statement: write the inputs into the instance,
    /// call `__body__(&instance)`, read the outputs.
    pub fn lower_fb_invocation(
        &self,
        func_call: hir::hir_def::expressions::expression::FuncCall<'db>,
        fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
    ) -> Result<Option<crate::stmt::MirStmt>, LowerTypeError> {
        use hir::hir_def::pous::variable::VariableKind;

        let path = func_call.path(self.db);

        // Get the instance variable name (the callee is a variable, not a type)
        let instance_ident = path
            .expr(self.db)
            .map(|pe| pe.ident(self.db).ident)
            .ok_or_else(|| LowerTypeError::UnsupportedType("FB call without name".to_string()))?;

        // In FB body context, the nested FB instance is a field of 'this', not a local.
        let instance = if let Some(ref this_struct) = self.this_struct {
            if let Some(field) = this_struct.fields.iter().find(|f| f.name == instance_ident) {
                MirPlace::ThisField {
                    field_name: instance_ident,
                    field_offset: field.offset,
                    field_type: field.ty.clone(),
                }
            } else {
                MirPlace::Local(instance_ident)
            }
        } else {
            MirPlace::Local(instance_ident)
        };

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

        // Build input writes from the call arguments
        let mut input_writes = Vec::new();
        let mut output_reads = Vec::new();

        // Map FB input variable names to field offsets
        let fb_vars: Vec<_> = fb.variables(self.db).to_vec();

        for param in func_call.params(self.db) {
            match param.kind(self.db) {
                ParamAssignKind::FormalInput {
                    param: param_ident,
                    value,
                } => {
                    let param_name = param_ident.ident;
                    // Find the field in the struct
                    if let Some(field) = struct_type.fields.iter().find(|f| f.name == param_name) {
                        let mir_elem = match &field.ty {
                            MirType::Elementary(e) => *e,
                            _ => continue, // skip non-elementary fields for now
                        };
                        let expr = self.lower_expr(value)?;
                        input_writes.push((field.offset, expr, mir_elem));
                    }
                }
                ParamAssignKind::NonFormal { value } => {
                    // Positional: match to next input variable
                    // Find the i-th input variable
                    let input_vars: Vec<_> = fb_vars
                        .iter()
                        .filter(|v| v.kind(self.db) == VariableKind::Input)
                        .collect();
                    let idx = input_writes.len();
                    if idx < input_vars.len() {
                        let var_name = input_vars[idx].name(self.db);
                        if let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name)
                        {
                            let mir_elem = match &field.ty {
                                MirType::Elementary(e) => *e,
                                _ => continue,
                            };
                            let expr = self.lower_expr(value)?;
                            input_writes.push((field.offset, expr, mir_elem));
                        }
                    }
                }
                ParamAssignKind::FormalOutput {
                    param: param_ident,
                    variable,
                    ..
                } => {
                    let param_name = param_ident.ident;
                    if let Some(field) = struct_type.fields.iter().find(|f| f.name == param_name) {
                        let mir_elem = match &field.ty {
                            MirType::Elementary(e) => *e,
                            _ => continue,
                        };
                        let place = self.lower_variable_access(variable)?;
                        output_reads.push((field.offset, place, mir_elem));
                    }
                }
            }
        }

        // Body function name. For generic FBs, use the per-variable
        // mangled name (e.g. `NsA.Counter$INT$__body__`); fall back to
        // the FB's namespace-qualified name for non-generic FBs.
        let mangled_root = self
            .local_fb_mangling
            .as_ref()
            .and_then(|m| m.get(&instance_ident).copied())
            .unwrap_or_else(|| {
                crate::lower::monomorphize::qualified_pou_ident(
                    self.db,
                    hir::hir_ty::ty::Type::FunctionBlock(fb),
                )
            });
        let body_func = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(format!(
                "{}$__body__",
                mangled_root.text(self.db)
            )),
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
            Type::Elementary(spec) if spec.is_any() => {
                if let Some(concrete) = self.any_override {
                    elementary_spec_to_mir(concrete)
                } else if let Some(concrete) = self.resolve_any_from_fb_subs(spec) {
                    elementary_spec_to_mir(concrete)
                } else {
                    elementary_spec_to_mir(spec)
                }
            }
            Type::Elementary(spec) => elementary_spec_to_mir(spec),
            Type::Enum(_) => {
                // Enums compare as their storage type
                Ok(MirElementary::DInt)
            }
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

    /// Try to resolve an ANY_* type from FB substitutions.
    /// Looks through all FB subs for a concrete type that matches the ANY_* group.
    fn resolve_any_from_fb_subs(&self, any_spec: ElementarySpec) -> Option<ElementarySpec> {
        let subs_map = self.fb_subs.as_ref()?;
        for fb_subs in subs_map.values() {
            for concrete in fb_subs.values() {
                if any_spec.accepts(*concrete) {
                    return Some(*concrete);
                }
            }
        }
        None
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
