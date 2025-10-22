use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{request::GotoDeclarationResponse, GotoDefinitionResponse, Hover},
};
use hir::{
    HirNodeInfo,
    hir_def::interned::namespace::SpanNamespaceAccess,
    hir_ty::name_res::resolve_namespace_access,
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for SpanNamespaceAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        match resolve_namespace_access(db, self.scope_id, self.path) {
            Some(resolved) => resolved.hover(db, resolved.name_span(db).start_byte),
            None => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match resolve_namespace_access(db, self.scope_id, self.path) {
            Some(resolved) => resolved.declaration(db),
            None => None,
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match resolve_namespace_access(db, self.scope_id, self.path) {
            Some(resolved) => resolved.definition(db),
            None => None,
        }
    }
}
