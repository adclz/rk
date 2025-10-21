use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{hir_def::interned::namespace::SpanNamespaceAccess, hir_ty::{expr_resolver::{ResolvedExpr, ResolvedExprKind}, name_res::resolve_namespace_access}, HirNodeInfo};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for SpanNamespaceAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        match resolve_namespace_access(db, self.scope_id, self.path) {
            Some(resolved) => resolved.hover(db, resolved.get_span(db).start_byte),
            None => None,
        }
    }
}
