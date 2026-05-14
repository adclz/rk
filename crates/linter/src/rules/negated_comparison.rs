use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        ComparisonOperatorKind, Expr, ExprKind, PrimaryExpr, UnaryOperatorKind,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "negated-comparison";

/// L0111: `NOT (x = y)` can be simplified to `x <> y`.
struct NegatedComparison;

impl ErrorCode for NegatedComparison {
    fn code(&self) -> &'static str {
        "L0111"
    }

    fn description(&self) -> &'static str {
        "negated comparison"
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
    } = expr.expr(db)
    {
        // Check NOT (comparison) or NOT comparison
        if let Some((op, lhs, rhs)) = unwrap_comparison(db, inner) {
            let inv = invert(op);

            let mut diag = diag()
                .message(format!(
                    "NOT with '{}' can be simplified to '{}'",
                    op.as_str(),
                    inv.as_str()
                ))
                .desc(&NegatedComparison)
                .range(expr.get_span(db))
                .severity(DiagnosticSeverity::INFORMATION)
                .call();

            diag.with_related(Related::new(
                format!(
                    "replace 'NOT {}' with '{} {} {}'",
                    inner.as_call_site(db).to_string(db),
                    lhs.as_call_site(db).to_string(db),
                    inv,
                    rhs.as_call_site(db).to_string(db)
                ),
                inner.get_scope_id(db).file(db),
                inner.get_span(db),
            ));

            diagnostics.push(diag);
        }
    }
}

/// Unwrap parentheses and check if the expression is a comparison.
fn unwrap_comparison<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
) -> Option<(ComparisonOperatorKind, Expr<'db>, Expr<'db>)> {
    match expr.expr(db) {
        ExprKind::ComparisonOperator {
            operator,
            left,
            right,
        } => Some((*operator, *left, *right)),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            unwrap_comparison(db, expr)
        }
        _ => None,
    }
}

fn invert(op: ComparisonOperatorKind) -> ComparisonOperatorKind {
    match op {
        ComparisonOperatorKind::Eq => ComparisonOperatorKind::Ne,
        ComparisonOperatorKind::Ne => ComparisonOperatorKind::Eq,
        ComparisonOperatorKind::Lt => ComparisonOperatorKind::Ge,
        ComparisonOperatorKind::Gt => ComparisonOperatorKind::Le,
        ComparisonOperatorKind::Le => ComparisonOperatorKind::Gt,
        ComparisonOperatorKind::Ge => ComparisonOperatorKind::Lt,
    }
}
