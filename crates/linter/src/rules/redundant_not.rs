use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind, UnaryOperatorKind},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "redundant-not";

/// L0108: double negation `NOT NOT x` can be simplified.
struct RedundantNot;

impl ErrorCode for RedundantNot {
    fn code(&self) -> &'static str {
        "L0108"
    }

    fn description(&self) -> &'static str {
        "redundant NOT"
    }
}

pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if let ExprKind::UnaryOperator {
            expr: inner,
            operator: UnaryOperatorKind::Not,
        } = expr.expr(db) {
        // Check for NOT NOT x
        if let ExprKind::UnaryOperator {
            operator: UnaryOperatorKind::Not,
            ..
        } = inner.expr(db)
        {
            diagnostics.push(
                diag()
                    .message(
                        "double negation: 'NOT NOT x' can be simplified to 'x'".to_string(),
                    )
                    .desc(&RedundantNot)
                    .range(expr.get_span(db))
                    .severity(DiagnosticSeverity::INFORMATION)
                    .call(),
            );
        }
        // Recurse into inner
    }
}
