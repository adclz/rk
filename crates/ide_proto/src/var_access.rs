use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind, Position, request::GotoDeclarationResponse
    },
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_def::expressions::expression::{InitExpr, VariableAccess},
    hir_ty::{body_inference::infer_body_scope, ty::Type},
};

use crate::{ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for VariableAccess<'db> {
    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.inlay_hint(db, self))
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.declaration(db))
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.hover(db, offset, self))
    }
}
