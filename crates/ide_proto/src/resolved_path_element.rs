use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{Hover, InlayHint, InlayHintKind, InlayHintLabel, Position},
};
use hir::{
    hir_ty::{
        ty::Ty, ty_var_access_resolver::{ResolvedPathElement, ResolvedPathElementKind},
    }, HirNodeInfo, TypeInfo
};

use crate::{ToProtocol, ty::TyHover};

impl<'db> ToProtocol<'db> for ResolvedPathElement<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match self.kind {
            ResolvedPathElementKind::Ty(ty) => ty.force_hover(db),
            _ => None,
        }
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        match self.kind {
            ResolvedPathElementKind::Ty(ty) => ty.declaration(db),
            _ => None,
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        match self.kind {
            ResolvedPathElementKind::Ty(ty) => ty.definition(db),
            _ => None,
        }
    }
}
