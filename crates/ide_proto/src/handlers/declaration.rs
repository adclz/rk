use auto_lsp::lsp_types::{Location, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, SpecKind, StructElement},
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{body::infer_body, name_res::resolve_namespace_access, ty::Type},
};

use crate::handlers::{DeclarationHandler, DefinitionHandler};

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
        match self.kind(db) {
            SpecKind::Target(target) => {
                let pou = resolve_namespace_access(db, &target.path)?;
                pou.definition(db)
            }
            SpecKind::Ref(_ref) => _ref.declaration(db),
            _ => Some(GotoDeclarationResponse::Scalar(Location::new(
                self.scope_id(db).file(db).url(db).to_owned(),
                self.get_span(db).into(),
            ))),
        }
    }
}

impl<'db> DeclarationHandler<'db> for BeginPathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DeclarationHandler<'db> for PathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_path_expr_with_adjustments(*self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DeclarationHandler<'db> for Expr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DeclarationHandler<'db> for VariableAccess<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
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
