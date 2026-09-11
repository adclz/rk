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
        "L0209"
    }

    fn description(&self) -> &'static str {
        "comparison with boolean literal"
    }
}

/// Check a single node (no recursion) for `x = TRUE`, `x = FALSE`, `x <> TRUE`, `x <> FALSE`.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::ComparisonOperator {
        left,
        operator,
        right,
    } = expr.expr(db)
    else {
        return;
    };
    if !matches!(
        operator,
        ComparisonOperatorKind::Eq | ComparisonOperatorKind::Ne
    ) {
        return;
    }

    let suggestion = if let Some(val) = is_bool_literal(db, right) {
        simplification(*operator, &val)
    } else if let Some(val) = is_bool_literal(db, left) {
        simplification(*operator, &val)
    } else {
        return;
    };

    diagnostics.push(
        diag()
            .message(format!(
                "comparison with boolean literal can be simplified to {suggestion}"
            ))
            .desc(&BoolComparison)
            .range(
                hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::INFORMATION)
            .call(),
    );
}

fn is_bool_literal<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> Option<String> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Bool(ident))) => {
            Some(ident.text(db).to_uppercase().to_string())
        }
        _ => None,
    }
}

fn simplification(op: ComparisonOperatorKind, bool_val: &str) -> &'static str {
    match (op, bool_val == "TRUE") {
        (ComparisonOperatorKind::Eq, true) => "the variable itself",
        (ComparisonOperatorKind::Eq, false) => "NOT variable",
        (ComparisonOperatorKind::Ne, true) => "NOT variable",
        (ComparisonOperatorKind::Ne, false) => "the variable itself",
        _ => unreachable!(),
    }
}
