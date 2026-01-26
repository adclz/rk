use auto_lsp::lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{
    hir_def::expressions::expression::BeginPathExpr, hir_ty::body::infer_body,
};

use crate::{to_proto::ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for BeginPathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }

    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.hover(db, offset, self))
    }
}
