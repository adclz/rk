use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::{Elementary, Expr, ExprKind, PrimaryExpr};

#[salsa::tracked]
pub fn resolve_range<'db>(db: &'db dyn BaseDatabase, range: Expr<'db>) -> Option<u64> {
    match range.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(v))) => v.as_u64(db).ok(),
        _ => None,
    }
}
