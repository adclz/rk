use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{hir_def::expressions::statement::{Stmt, StmtKind}};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Stmt<'db> {
    fn completion(
            &'db self,
            db: &'db dyn BaseDatabase,
            offset: usize,
        ) -> Option<Vec<CompletionItem>> {
        None
    }
}
