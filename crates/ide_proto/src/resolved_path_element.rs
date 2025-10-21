use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, request::GotoDeclarationResponse,
    },
};
use hir::hir_ty::walk::{ResolvedPath, ResolvedPathKind};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedPath<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match &self.kind {
            ResolvedPathKind::Pou(p) => p.hover(db, offset),
            ResolvedPathKind::Spec(t) => t.hover(db, offset),
            ResolvedPathKind::StructElement(st) => st.hover(db, offset),
            ResolvedPathKind::Variable(v) => v.hover(db, offset),
            ResolvedPathKind::Method(m) => m.hover(db, offset),
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match &self.kind {
            ResolvedPathKind::Pou(p) => p.declaration(db),
            ResolvedPathKind::Spec(t) => t.declaration(db),
            ResolvedPathKind::StructElement(st) => st.declaration(db),
            ResolvedPathKind::Variable(v) => v.declaration(db),
            ResolvedPathKind::Method(m) => m.declaration(db),
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match &self.kind {
            ResolvedPathKind::Pou(p) => p.definition(db),
            ResolvedPathKind::Spec(t) => t.definition(db),
            ResolvedPathKind::StructElement(st) => st.definition(db),
            ResolvedPathKind::Variable(v) => v.definition(db),
            ResolvedPathKind::Method(m) => m.definition(db),
        }
    }
}
