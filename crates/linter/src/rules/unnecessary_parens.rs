use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "unnecessary-parens";

/// L0132: unnecessary parentheses around a simple expression.
struct UnnecessaryParens;

impl ErrorCode for UnnecessaryParens {
    fn code(&self) -> &'static str {
        "L0132"
    }

    fn description(&self) -> &'static str {
        "unnecessary parentheses"
    }
}

/// Check a single node for unnecessary parentheses.
/// Flags `(x)` where `x` is a simple literal, variable, or enum value.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) = expr.expr(db)
    else {
        return;
    };

    // Only flag if the inner expression is "simple" - a literal, variable, or enum value.
    // Parentheses around compound expressions like (a + b) may be intentional for clarity.
    let is_simple = matches!(
        inner.expr(db),
        ExprKind::PrimaryExpr(
            PrimaryExpr::Literal(_)
                | PrimaryExpr::VariableAccess(_)
                | PrimaryExpr::EnumValue { .. }
        )
    );

    if is_simple {
        diagnostics.push(
            diag()
                .message(format!(
                    "unnecessary parentheses around '{}'",
                    inner.as_call_site(db).to_string(db)
                ))
                .desc(&UnnecessaryParens)
                .range(expr.get_span(db))
                .severity(DiagnosticSeverity::INFORMATION)
                .call(),
        );
    }
}
