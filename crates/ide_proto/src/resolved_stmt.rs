use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::hir_ty::stmt_resolver::{ResolvedStmt, ResolvedStmtKind};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedStmt<'db> {
    fn completion(
            &'db self,
            db: &'db dyn BaseDatabase,
            offset: usize,
        ) -> Option<Vec<CompletionItem>> {
        match self.kind(db) {
            ResolvedStmtKind::EmptyPathExpression(p) => p.completion(db, offset),
            _ => None,
        }
    }
}
