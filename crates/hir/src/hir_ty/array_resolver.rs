use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::expressions::expression::{Elementary, Expr},
    hir_ty::expr_resolver::{ResolvedExprKind, resolve_expr},
};

#[salsa::tracked]
pub fn resolve_range<'db>(db: &'db dyn BaseDatabase, range: Expr<'db>) -> Option<u64> {
    match resolve_expr(db, range).kind(db) {
        ResolvedExprKind::Literal(Elementary::InferInteger(v)) => v.as_u64(db).ok(),
        _ => None,
    }
}
