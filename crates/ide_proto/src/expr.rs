use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel,
        MarkupContent, MarkupKind, request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr, RefValue},
    hir_ty::{body_inference::infer_body_scope, ty::Type},
};

use crate::{HasComment, ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for Expr<'db> {
    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_expr_with_adjustments(db, *self)
            .and_then(|v| v.inlay_hint(db, self))
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.hover(db, offset, self))
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.declaration(db))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}
