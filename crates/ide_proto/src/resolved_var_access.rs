use std::fmt::format;

use ast::generated::Target;
use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent, MarkupKind,
        request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_def::{comment_index::comment_index, pous::variable::VariableKind},
    hir_ty::{
        ty::TyKind,
        ty_var_access_resolver::ResolvedAccess,
        walk::{ResolvedPath, ResolvedPathKind, ResolvedPathResult},
    },
};

use crate::{HasComment, ToProtocol};

impl<'db> ToProtocol<'db> for ResolvedAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        self.resolved(db).ok()?.hover(db, offset)
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        self.resolved(db).ok()?.declaration(db)
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        self.resolved(db).ok()?.definition(db)
    }
}
