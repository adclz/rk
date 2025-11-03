
use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        request::GotoDeclarationResponse, CompletionItem, GotoDefinitionResponse, Hover
    },
};
use hir::{hir_ty::{name_res::global_pou_index, ty_var_access_resolver::ResolvedAccess}, HirNodeInfo};

use crate::{completions::per_scope::scoped_completions, ToProtocol};

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
            Err(_) => scoped_completions(db, self.get_scope_id(db), offset)
        }
    }
}
