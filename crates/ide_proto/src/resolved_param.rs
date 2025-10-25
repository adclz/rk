use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        InlayHint, InlayHintKind, InlayHintLabel, Position,
    },
};
use hir::{
    hir_ty::{
        param_resolver::{ResolvedParam, ResolvedParamKind}, ty::Ty
    }, HirNodeInfo, TypeInfo
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedParam<'db> {
    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        Some(InlayHint {
            position: get_param_inlay_hint_position(db, self)?,
            label: InlayHintLabel::String(match get_param_ty(db, self) {
                Some(ty) => format!(": {}", ty.type_name(db)),
                None => return None,
            }),
            kind: Some(InlayHintKind::PARAMETER),
            padding_left: Some(false),
            padding_right: Some(false),
            text_edits: None,
            tooltip: None,
            data: None,
        })
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<auto_lsp::lsp_types::Hover> {
        match self.kind {
            ResolvedParamKind::NonFormal { resolved_param, .. } => {
                resolved_param.and_then(|p| p.hover(db, offset))
            },
            ResolvedParamKind::FormalInput { resolved_param, .. } => {
                resolved_param.and_then(|p| p.hover(db, offset))
            }
            ResolvedParamKind::FormalOutput { resolved_param, .. } => {
                resolved_param.and_then(|p| p.hover(db, offset))
            }
        }
    }
}

pub fn get_param_inlay_hint_position(
    db: &dyn BaseDatabase,
    param: &ResolvedParam,
) -> Option<Position> {
    match param.kind {
        ResolvedParamKind::NonFormal { .. } => None,
        ResolvedParamKind::FormalInput { param, .. } => Some(param.get_span(db).lsp().end),
        ResolvedParamKind::FormalOutput { param, .. } => Some(param.get_span(db).lsp().end),
    }
}

pub fn get_param_ty<'db>(db: &'db dyn BaseDatabase, param: &'db ResolvedParam) -> Option<Ty<'db>> {
    match param.kind {
        ResolvedParamKind::NonFormal { .. } => None,
        ResolvedParamKind::FormalInput { resolved_param, .. } => {
            resolved_param.and_then(|p| Some(p.spec(db).to_ty(db)))
        }
        ResolvedParamKind::FormalOutput { resolved_param, .. } => {
            resolved_param.and_then(|p| Some(p.spec(db).to_ty(db)))
        }
    }
}
