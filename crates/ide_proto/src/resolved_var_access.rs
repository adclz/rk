use auto_lsp::{
    core::semantic_tokens_builder::SemanticTokensBuilder, default::db::{BaseDatabase, tracked::get_ast}, lsp_types::{CompletionItem, GotoDefinitionResponse, Hover, request::GotoDeclarationResponse}
};
use hir::{
    HirNodeInfo,
    hir_ty::ty_var_access_resolver::ResolvedAccess,
};

use crate::{
    ToProtocol,
    completions::context::ScopeCompletionCtx,
};

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

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.resolved(db) {
            Ok(resolved) => resolved.completion(db, offset),
            Err(_) => {
                let file = self.get_scope_id(db).file(db);
                let node = &get_ast(db, file)[self.call_site.id.id()];
                let query = node.get_text(file.document(db).as_bytes()).ok()?;

                Some(
                    ScopeCompletionCtx::new(self.get_scope_id(db), offset, query)
                        .scoped(db)
                        .take_items(),
                )
            }
        }
    }

    fn semantic_tokens(&'db self, db: &'db dyn BaseDatabase, builder: &mut SemanticTokensBuilder) {
        match self.resolved(db) {
            Ok(resolved) => resolved.semantic_tokens(db, builder),
            Err(_) => {}
        }
    }
}
