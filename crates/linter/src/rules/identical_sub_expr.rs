use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        BooleanOperatorKind, Expr, ExprKind, PrimaryExpr, VariableAccessKind,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "identical-sub-expr";

/// L0121: identical subexpressions on both sides of a boolean operator.
struct IdenticalSubExpr;

impl ErrorCode for IdenticalSubExpr {
    fn code(&self) -> &'static str {
        "L0121"
    }

    fn description(&self) -> &'static str {
        "identical subexpressions"
    }
}

/// Check an expression tree for `a AND a`, `a OR a`, `a XOR a`.
pub fn check_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    match expr.expr(db) {
        ExprKind::BooleanOperator {
            left,
            operator,
            right,
        } => {
            check_expr(db, body, left, diagnostics);
            check_expr(db, body, right, diagnostics);

            if same_expr(db, body, left, right) {
                let op = operator.as_str();
                let hint = match operator {
                    BooleanOperatorKind::And => "result is always the same as either operand",
                    BooleanOperatorKind::Or => "result is always the same as either operand",
                    BooleanOperatorKind::Xor => "result is always FALSE",
                    _ => "result may be unintended",
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
        }
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            check_expr(db, body, left, diagnostics);
            check_expr(db, body, right, diagnostics);
        }
        ExprKind::UnaryOperator { expr, .. } => {
            check_expr(db, body, expr, diagnostics);
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            check_expr(db, body, expr, diagnostics);
        }
        _ => {}
    }
}

/// Structurally compare two expressions. Returns true if they reference the same variable.
fn same_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: &Expr<'db>,
    right: &Expr<'db>,
) -> bool {
    match (left.expr(db), right.expr(db)) {
        // Both are simple variable accesses - compare resolved variables
        (
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(la)),
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(ra)),
        ) => {
            let lv = resolve_var(db, body, la);
            let rv = resolve_var(db, body, ra);
            lv.is_some() && lv == rv
        }
        // Both parenthesized - unwrap
        (
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: le }),
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: re }),
        ) => same_expr(db, body, le, re),
        // One parenthesized, one not
        (ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: le }), _) => {
            same_expr(db, body, le, right)
        }
        (_, ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: re })) => {
            same_expr(db, body, left, re)
        }
        // Both are NOT expr - compare inner
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

fn resolve_var<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    access: &hir::hir_def::expressions::expression::VariableAccess<'db>,
) -> Option<hir::hir_def::pous::variable::VariableDecl<'db>> {
    let VariableAccessKind::Symbolic(begin) = access.kind(db) else {
        return None;
    };
    let path = begin.expr(db)?;
    let Type::Variable((var, _)) = body.type_of_path_expr.get(&path)? else {
        return None;
    };
    Some(*var)
}
