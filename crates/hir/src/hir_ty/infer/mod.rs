use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::{BeginPathExpr, Expr, InitExpr, PathExpr, VariableAccess},
        spec::Spec,
    },
    hir_ty::{body::infer_body, signature::infer_signature, ty::Type},
};

pub mod coerce;
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
        infer_signature(db, self.get_scope_id(db))
            .init_expr_result
            .type_of_init_expr
            .get(self)
            .copied()
            .unwrap_or_default()
    }
}

impl<'db> Infer<'db> for VariableAccess<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_body(db, self.get_scope_id(db)).get_type_of_variable_access(db, *self)
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
        let head = infer_signature(db, self.get_scope_id(db))
            .body_infer_result
            .get_type_of_expr(*self);
        if head.is_never() {
            infer_body(db, self.get_scope_id(db)).get_type_of_expr(*self)
        } else {
            head
        }
    }
}
