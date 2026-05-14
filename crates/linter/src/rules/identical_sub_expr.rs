use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        BooleanOperatorKind, Expr, ExprKind, PrimaryExpr,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "identical-sub-expr";

struct IdenticalSubExpr;

impl ErrorCode for IdenticalSubExpr {
    fn code(&self) -> &'static str {
        "L0311"
    }

    fn description(&self) -> &'static str {
        "identical subexpressions"
    }
}

/// Check a single node for `a AND a`, `a OR a`, `a XOR a`. No recursion.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::BooleanOperator {
        left,
        operator,
        right,
    } = expr.expr(db)
    else {
        return;
    };
    if !same_expr(db, body, left, right) {
        return;
    }
    let op = operator.as_str();
    let hint = match operator {
        BooleanOperatorKind::And => "result is always the same as either operand",
        BooleanOperatorKind::Or => "result is always the same as either operand",
        BooleanOperatorKind::Xor => "result is always FALSE",
    };
    diagnostics.push(
        diag()
            .message(format!(
                "identical expressions on both sides of '{op}', {hint}"
            ))
            .desc(&IdenticalSubExpr)
            .range(expr.get_span(db))
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

fn same_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: &Expr<'db>,
    right: &Expr<'db>,
) -> bool {
    match (left.expr(db), right.expr(db)) {
        (
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(la)),
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(ra)),
        ) => {
            match (
                body.type_of_variable_access_with_adjustments(db, *la),
                body.type_of_variable_access_with_adjustments(db, *ra),
            ) {
                (Type::Variable((l_var, _)), Type::Variable((r_var, _))) => l_var == r_var,
                _ => false,
            }
        }
        (
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: le }),
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: re }),
        ) => same_expr(db, body, le, re),
        (ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: le }), _) => {
            same_expr(db, body, le, right)
        }
        (_, ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: re })) => {
            same_expr(db, body, left, re)
        }
        (
            ExprKind::UnaryOperator {
                expr: le,
                operator: lo,
            },
            ExprKind::UnaryOperator {
                expr: re,
                operator: ro,
            },
        ) => lo == ro && same_expr(db, body, le, re),
        _ => false,
    }
}
