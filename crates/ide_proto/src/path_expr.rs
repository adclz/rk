use auto_lsp::lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{hir_def::expressions::expression::PathExpr, hir_ty::body_inference::infer_body_scope};

use crate::{to_proto::ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for PathExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_path_expr
            .get(self)
            .and_then(|typ| typ.hover(db, offset, self))
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_path_expr
            .get(self)
            .and_then(|typ| typ.declaration(db))
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_path_expr
            .get(self)
            .and_then(|typ| typ.definition(db))
    }
}
