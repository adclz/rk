use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        ComparisonOperatorKind, Elementary, Expr, ExprKind, PrimaryExpr,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "bool-comparison";

struct BoolComparison;

impl ErrorCode for BoolComparison {
    fn code(&self) -> &'static str {
        "L0119"
    }

    fn description(&self) -> &'static str {
        "comparison with boolean literal"
    }
}

/// Check a single expression for `x = TRUE`, `x = FALSE`, `x <> TRUE`, `x <> FALSE`.
pub fn check_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    match expr.expr(db) {
        ExprKind::ComparisonOperator {
            left,
            operator,
            right,
        } => {
            // Recurse into subexpressions
            check_expr(db, left, diagnostics);
            check_expr(db, right, diagnostics);

            // Only = and <> are relevant for bool comparison
            if !matches!(operator, ComparisonOperatorKind::Eq | ComparisonOperatorKind::Ne) {
                return;
            }

            let (suggestion, span) =
                if let Some(val) = is_bool_literal(db, right) {
                    let s = simplification(*operator, &val);
                    (s, expr.get_span(db))
                } else if let Some(val) = is_bool_literal(db, left) {
                    let s = simplification(*operator, &val);
                    (s, expr.get_span(db))
                } else {
                    return;
                };

            diagnostics.push(
                diag()
                    .message(format!("comparison with boolean literal can be simplified to {suggestion}"))
                    .desc(&BoolComparison)
                    .range(span)
                    .severity(DiagnosticSeverity::INFORMATION)
                    .call(),
            );
        }
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            check_expr(db, left, diagnostics);
            check_expr(db, right, diagnostics);
        }
        ExprKind::UnaryOperator { expr, .. } => {
            check_expr(db, expr, diagnostics);
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            check_expr(db, expr, diagnostics);
        }
        _ => {}
    }
}

fn is_bool_literal<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> Option<String> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Bool(ident))) => {
            Some(ident.text(db).to_uppercase().to_string())
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            is_bool_literal(db, expr)
        }
        _ => None,
    }
}

/// Returns the simplified form as a hint string.
fn simplification(op: ComparisonOperatorKind, bool_val: &str) -> &'static str {
    match (op, bool_val == "TRUE") {
        (ComparisonOperatorKind::Eq, true) => "the variable itself",   // x = TRUE  -  x
        (ComparisonOperatorKind::Eq, false) => "NOT variable",         // x = FALSE -  NOT x
        (ComparisonOperatorKind::Ne, true) => "NOT variable",          // x <> TRUE -  NOT x
        (ComparisonOperatorKind::Ne, false) => "the variable itself",  // x <> FALSE - x
        _ => unreachable!(),
    }
}
