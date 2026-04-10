use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        AddOperatorKind, Expr, ExprKind, PrimaryExpr, VariableAccessKind,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "sub-self";

/// L0313: subtracting a variable from itself is always 0.
struct SubSelf;

impl ErrorCode for SubSelf {
    fn code(&self) -> &'static str {
        "L0313"
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
        } = expr.expr(db) {
        if let Some(name) = same_variable(db, body, left, right) {
            diagnostics.push(
                diag()
                    .message(format!(
                        "'{name}' is subtracted from itself, result is always 0"
                    ))
                    .desc(&SubSelf)
                    .range(expr.get_span(db))
                    .severity(DiagnosticSeverity::WARNING)
                    .call(),
            );
        }
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
