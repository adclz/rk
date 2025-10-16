use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind, Position},
};
use hir::{
    hir_ty::{
        ty::{Ty, TyKind}, walk::ResolvedPath,
    }, HirNodeInfo, TypeInfo
};

use crate::{ToProtocol};

impl<'db> ToProtocol<'db> for ResolvedPath<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        None
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        None
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        None
    }
}
