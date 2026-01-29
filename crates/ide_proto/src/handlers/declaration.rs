use auto_lsp::lsp_types::{Location, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{body::infer_body, signature::infer_signature, ty::Type},
};

use crate::handlers::DeclarationHandler;

impl<'db> DeclarationHandler<'db> for VariableDecl<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DeclarationHandler<'db> for StructElement<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DeclarationHandler<'db> for Spec<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_signature(db, self.scope_id(db));
        let typ = infer.type_of_specs.get(self)?;
        typ.declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for BeginPathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_begin_path_expr(db, *self)
            .and_then(|typ| typ.declaration(db))
    }
}

impl<'db> DeclarationHandler<'db> for PathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_path_expr(db, *self)
            .and_then(|typ| typ.declaration(db))
    }
}

impl<'db> DeclarationHandler<'db> for Expr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        if let Some(r) = infer_signature(db, self.scope_id(db))
            .body_infer_result
            .get_type_of_expr(*self)
            .and_then(|typ| typ.declaration(db))
        {
            return Some(r);
        }

        infer_body(db, self.scope_id(db))
            .get_type_of_expr(*self)
            .and_then(|typ| typ.declaration(db))
    }
}

impl<'db> DeclarationHandler<'db> for VariableAccess<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_variable_access(db, *self)
            .and_then(|typ| typ.declaration(db))
    }
}

impl<'db> DeclarationHandler<'db> for ParamAssign<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .variable_of_param
            .get(self)
            .and_then(|var| Type::new_var(db, *var).declaration(db))
    }
}

impl<'db> DeclarationHandler<'db> for Type<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        match self {
            Type::Variable((var, _multibits)) => var.declaration(db),
            _ => None,
        }
    }
}
