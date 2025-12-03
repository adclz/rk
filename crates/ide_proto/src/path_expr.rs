use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, HoverContents, MarkupContent, MarkupKind,
        request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind, PathExpr, PrimaryExpr, RefValue},
    hir_ty::body_inference::infer_body_scope,
};

use crate::{to_proto::ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for PathExpr<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_path_expr_with_adjustments(*self)
            .and_then(|typ| typ.hover(db, offset, self))
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_path_expr_with_adjustments(*self)
            .and_then(|typ| typ.declaration(db))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_path_expr_with_adjustments(*self)
            .and_then(|typ| typ.definition(db))
    }
}
