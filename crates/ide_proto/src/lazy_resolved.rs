use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::Hover,
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
            Some(resolved) => resolved.hover(db, resolved.get_span(db).start_byte),
            None => None,
        }
    }
}
