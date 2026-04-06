use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        Expr, ExprKind, PrimaryExpr, UnaryOperatorKind,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "redundant-not";

/// L0122: double negation `NOT NOT x` can be simplified.
struct RedundantNot;

impl ErrorCode for RedundantNot {
    fn code(&self) -> &'static str {
        "L0122"
    }

    fn description(&self) -> &'static str {
        "redundant NOT"
    }
}

pub fn check_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    match expr.expr(db) {
        ExprKind::UnaryOperator {
            expr: inner,
            operator: UnaryOperatorKind::Not,
        } => {
            // Check for NOT NOT x
            if let ExprKind::UnaryOperator {
                operator: UnaryOperatorKind::Not,
                ..
            } = inner.expr(db)
            {
                diagnostics.push(
                    diag()
                        .message("double negation: 'NOT NOT x' can be simplified to 'x'".to_string())
                        .desc(&RedundantNot)
                        .range(expr.get_span(db))
                        .severity(DiagnosticSeverity::INFORMATION)
                        .call(),
                );
            }
            // Recurse into inner
            check_expr(db, inner, diagnostics);
        }
        ExprKind::UnaryOperator { expr: inner, .. } => {
            check_expr(db, inner, diagnostics);
        }
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            check_expr(db, left, diagnostics);
            check_expr(db, right, diagnostics);
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            check_expr(db, expr, diagnostics);
        }
        _ => {}
    }
}
