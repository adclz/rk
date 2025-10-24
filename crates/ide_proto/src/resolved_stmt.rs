use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{hir_def::expressions::statement::{Stmt, StmtKind}, hir_ty::ty_var_access_resolver::resolve_local_path_expr};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Stmt<'db> {
    fn completion(
            &'db self,
            db: &'db dyn BaseDatabase,
            offset: usize,
        ) -> Option<Vec<CompletionItem>> {
        match self.stmt(db) {
            StmtKind::EmptyPathExpression(p) => resolve_local_path_expr(db, *p).completion(db, offset),
            _ => None,
        }
    }
}
