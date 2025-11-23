use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        InlayHint, InlayHintKind, InlayHintLabel, Position, request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo, TypeInfo, hir_def::expressions::expression::InitExpr, hir_ty::ty::Type
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for InitExpr<'db> {
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
        None
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        None
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        None
    }
}

pub fn get_expr_inlay_hint_position(
    db: &dyn BaseDatabase,
    param: &InitExpr,
) -> Option<Position> {
    None
}

pub fn get_expr_ty<'db>(
    db: &'db dyn BaseDatabase,
    param: &'db InitExpr,
) -> Option<Type<'db>> {
    None
}
