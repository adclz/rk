use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        ComparisonOperatorKind, Expr, ExprKind, PrimaryExpr, VariableAccessKind,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "self-comparison";

/// L0310: variable is compared to itself.
struct SelfComparison;

impl ErrorCode for SelfComparison {
    fn code(&self) -> &'static str {
        "L0310"
    }

    fn description(&self) -> &'static str {
        "self-comparison"
    }
}

/// Check a comparison expression for `x = x`, `x <> x`, `x > x`, etc.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if let ExprKind::ComparisonOperator {
        left,
        operator,
        right,
    } = expr.expr(db)
        && let Some(var_name) = same_variable(db, body, left, right) {
            let op = operator.as_str();
            let result_hint = match operator {
                ComparisonOperatorKind::Eq
                | ComparisonOperatorKind::Le
                | ComparisonOperatorKind::Ge => "always TRUE",
                ComparisonOperatorKind::Ne
                | ComparisonOperatorKind::Lt
                | ComparisonOperatorKind::Gt => "always FALSE",
            };
            diagnostics.push(
                diag()
                    .message(format!(
                        "'{var_name}' is compared to itself with '{op}', result is {result_hint}"
                    ))
                    .desc(&SelfComparison)
                    .range(hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::WARNING)
                    .call(),
            );
        }
}

/// If both expressions resolve to the same variable, return its name.
fn same_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: &Expr<'db>,
    right: &Expr<'db>,
) -> Option<String> {
    let lhs_var = resolve_variable(db, body, left)?;
    let rhs_var = resolve_variable(db, body, right)?;

    if lhs_var != rhs_var {
        return None;
    }

    Some(lhs_var.name(db).text(db).to_string())
}

fn resolve_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    expr: &Expr<'db>,
) -> Option<hir::hir_def::pous::variable::VariableDecl<'db>> {
    let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) = expr.expr(db) else {
        return None;
    };
    let VariableAccessKind::Symbolic(begin) = access.kind(db) else {
        return None;
    };
    let path = begin.expr(db)?;
    let Type::Variable((var, _)) = body.type_of_path_expr.get(&path)? else {
        return None;
    };
    Some(*var)
}
