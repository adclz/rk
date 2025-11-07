use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{
    HirNodeInfo,
    hir_def::{
        interned::namespace::SpanNamespaceAccess,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::name_res::{global_pou_index, resolve_namespace_access},
};

use crate::{
    ToProtocol,
};

impl<'db> ToProtocol<'db> for SpanNamespaceAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        match resolve_namespace_access(db, &self.path) {
            Some(resolved) => resolved.hover(db, resolved.name_span(db).start_byte),
            None => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match resolve_namespace_access(db, &self.path) {
            Some(resolved) => resolved.declaration(db),
            None => None,
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match resolve_namespace_access(db, &self.path) {
            Some(resolved) => resolved.definition(db),
            None => None,
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        eprintln!("FROM LAZY RESOLVED NAMESPACE ACCESS");
        //scoped_completions(db, self.scope_id, offset)
        None
    }
}

