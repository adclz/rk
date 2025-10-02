use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, InlayHint, InlayHintKind, InlayHintLabel, Position,
        request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_ty::{
        func_call_resolver::{ResolvedParam, ResolvedParamKind},
        ty::Ty,
    },
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

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: Option<usize>) -> Option<Hover> {
        get_param_ty(db, self).and_then(|ty| ty.hover(db, offset))
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        get_param_ty(db, self).and_then(|ty| ty.declaration(db))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        get_param_ty(db, self).and_then(|ty| ty.definition(db))
    }
}

pub fn get_param_inlay_hint_position(
    db: &dyn BaseDatabase,
    param: &ResolvedParam,
) -> Option<Position> {
    match param.kind(db) {
        ResolvedParamKind::NonFormal { .. } => None,
        ResolvedParamKind::FormalInput { param, .. } => Some(param.get_span(db).lsp().end),
        ResolvedParamKind::FormalOutput { param, .. } => Some(param.get_span(db).lsp().end),
    }
}

pub fn get_param_ty<'db>(db: &'db dyn BaseDatabase, param: &'db ResolvedParam) -> Option<Ty<'db>> {
    match param.kind(db) {
        ResolvedParamKind::NonFormal { .. } => None,
        ResolvedParamKind::FormalInput { resolved_param, .. } => {
            resolved_param.and_then(|p| p.ty(db).ok())
        }
        ResolvedParamKind::FormalOutput { resolved_param, .. } => {
            resolved_param.and_then(|p| p.ty(db).ok())
        }
    }
}
