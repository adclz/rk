use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        AddOperatorKind, Expr, ExprKind, PrimaryExpr, VariableAccessKind,
    },
    hir_ty::{body::BodyInferenceResult, infer::Infer, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "sub-self";

/// L0106: subtracting a variable from itself is always 0.
struct SubSelf;

impl ErrorCode for SubSelf {
    fn code(&self) -> &'static str {
        "L0106"
    }

    fn description(&self) -> &'static str {
        "subtraction from self"
    }
}

pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if let ExprKind::AddOperator {
        left,
        operator: AddOperatorKind::Minus,
        right,
    } = expr.expr(db)
        && let Some(name) = same_variable(db, body, left, right)
    {
        diagnostics.push(
            diag()
                .message(format!(
                    "'{name}' is subtracted from itself, result is always 0"
                ))
                .desc(&SubSelf)
                .range(
                    hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}

fn same_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: &Expr<'db>,
    right: &Expr<'db>,
) -> Option<String> {
    let lv = resolve_var(db, body, left)?;
    let rv = resolve_var(db, body, right)?;
    if lv != rv {
        return None;
    }
    // A float minus itself is NaN for a NaN or an infinity: that difference
    // is a finiteness test, not always 0, and calling it one sent the check
    // away as a mistake.
    if lv.spec(db).infer(db).is_float() {
        return None;
    }
    Some(lv.name(db).text(db).to_string())
}

fn resolve_var<'db>(
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
