use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
        invocation::Invocation,
        spec::Spec,
    },
    hir_ty::{
        body::infer_body,
        head::{init_inference::infer_initialization, signature::infer_signature},
        ty::Type,
    },
};

pub mod cast;
pub mod coerce;
pub mod const_eval;
pub mod expr;
pub mod literals;
pub mod normalize;
pub mod table;

pub trait Infer<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db>;
}

impl<'db> Infer<'db> for Spec<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_signature(db, self.get_scope_id(db))
            .type_of_specs
            .get(self)
            .copied()
            .unwrap_or_default()
    }
}

impl<'db> Infer<'db> for InitExpr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_initialization(db, self.get_scope_id(db))
            .init_expr_result
            .type_of_init_expr
            .get(self)
            .copied()
            .unwrap_or_default()
    }
}

impl<'db> Infer<'db> for Invocation<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_body(db, self.get_scope_id(db)).get_type_of_invocation(db, *self)
    }
}

impl<'db> Infer<'db> for ParamAssign<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match infer_body(db, self.get_scope_id(db)).variable_for_param(*self) {
            Some(var) => Type::new_var(db, var),
            None => Type::Never,
        }
    }
}

impl<'db> Infer<'db> for VariableAccess<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        // The head first, like an expression: a name inside a spec — an array
        // bound, a STRING length — is resolved while the head is inferred and
        // never appears in a body, so asking the body alone answered nothing.
        let head = infer_initialization(db, self.get_scope_id(db))
            .body_infer_result
            .get_type_of_variable_access(db, *self);
        if head.is_never() {
            infer_body(db, self.get_scope_id(db)).get_type_of_variable_access(db, *self)
        } else {
            head
        }
    }
}

impl<'db> Infer<'db> for BeginPathExpr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_body(db, self.get_scope_id(db)).get_type_of_begin_path_expr(db, *self)
    }
}

impl<'db> Infer<'db> for PathExpr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_body(db, self.get_scope_id(db)).get_type_of_path_expr(db, *self)
    }
}

impl<'db> Infer<'db> for Expr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        let head = infer_initialization(db, self.get_scope_id(db))
            .body_infer_result
            .get_type_of_expr(*self);
        if head.is_never() {
            infer_body(db, self.get_scope_id(db)).get_type_of_expr(*self)
        } else {
            head
        }
    }
}

impl<'db> Expr<'db> {
    /// The expression's type WITH its adjustments applied.
    ///
    /// Indexing, field access and dereference are recorded as adjustments over
    /// a base type, so the plain [`Infer::infer`] type of `arr[0]` is the ARRAY,
    /// not its element. Consumers that want the type an expression actually
    /// *evaluates to* — codegen picking a machine type, overload resolution
    /// classifying an argument — must use this, or they have to reconstruct the
    /// adjustment themselves.
    pub fn infer_adjusted(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        let head = infer_initialization(db, self.get_scope_id(db));
        let from_head = head
            .body_infer_result
            .type_of_expr_with_adjustments(db, *self);
        if from_head.is_never() {
            infer_body(db, self.get_scope_id(db)).type_of_expr_with_adjustments(db, *self)
        } else {
            from_head
        }
    }
}
