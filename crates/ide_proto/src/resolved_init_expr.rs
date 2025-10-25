use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        InlayHint, InlayHintKind, InlayHintLabel, Position, request::GotoDeclarationResponse,
    },
};
use hir::{
    hir_ty::{
        init_expr_resolver::{ResolvedInitExpr, ResolvedInitExprKind}, ty::Ty
    }, HirNodeInfo, TypeInfo
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedInitExpr<'db> {
    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        Some(InlayHint {
            position: get_expr_inlay_hint_position(db, self)?,
            label: InlayHintLabel::String(match get_expr_ty(db, self) {
                Some(ty) => format!(": {}", ty.type_name(db)),
                None => return None,
            }),
            kind: Some(InlayHintKind::TYPE),
            padding_left: Some(false),
            padding_right: Some(false),
            text_edits: None,
            tooltip: None,
            data: None,
        })
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match self.kind(db) {
            ResolvedInitExprKind::ConstantExpr(expr) => expr.declaration(db),
            ResolvedInitExprKind::StructElement { field, value } => field.declaration(db),
            _ => None,
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        match self.kind(db) {
            ResolvedInitExprKind::ConstantExpr(expr) => expr.definition(db),
            ResolvedInitExprKind::StructElement { field, value } => field.definition(db),
            _ => None,
        }
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        match self.kind(db) {
            ResolvedInitExprKind::ConstantExpr(expr) => expr.hover(db, offset),
            ResolvedInitExprKind::StructElement { field, .. } => field.hover(db, offset),
            _ => None,
        }
    }
}

pub fn get_expr_inlay_hint_position(
    db: &dyn BaseDatabase,
    param: &ResolvedInitExpr,
) -> Option<Position> {
    match param.kind(db) {
        ResolvedInitExprKind::StructElement { field, .. } => Some(field.get_span(db).lsp().end),
        _ => None,
    }
}

pub fn get_expr_ty<'db>(
    db: &'db dyn BaseDatabase,
    param: &'db ResolvedInitExpr,
) -> Option<Ty<'db>> {
    match param.kind(db) {
        ResolvedInitExprKind::StructElement { field, .. } => field.try_to_ty(db).ok(),
        _ => None,
    }
}
