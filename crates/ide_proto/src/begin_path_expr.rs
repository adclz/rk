use auto_lsp::{default::db::BaseDatabase, lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse}};
use hir::{hir_def::expressions::expression::BeginPathExpr, hir_ty::body_inference::infer_body_scope};

use crate::{ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for BeginPathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.hover(db, offset, self))
    }
}