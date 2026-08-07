use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "yoda-condition";

/// L0209: literal on the left side of a comparison.
struct YodaCondition;

impl ErrorCode for YodaCondition {
    fn code(&self) -> &'static str {
        "L0209"
    }

    fn description(&self) -> &'static str {
        "yoda condition"
    }
}

/// Check a single comparison node for literal-on-left pattern.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::ComparisonOperator {
        left,
        right,
        operator,
    } = expr.expr(db)
    else {
        return;
    };

    if is_literal(db, left) && !is_literal(db, right) {
        let mut diag = diag()
            .message("literal value on the left side of comparison".to_string())
            .desc(&YodaCondition)
            .range(
                hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::HINT)
            .call();

        diag.with_related(Related::new(
            format!(
                "swap both operands: '{} {} {}'",
                right.as_call_site(db).to_string(db),
                operator,
                left.as_call_site(db).to_string(db)
            ),
            expr.get_scope_id(db).file(db),
            expr.get_span(db),
        ));

        diagnostics.push(diag);
    }
}

fn is_literal<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(_)) => true,
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => is_literal(db, expr),
        _ => false,
    }
}
