use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::expression::{VariableAccess, VariableAccessKind},
        pous::variable::VariableDecl,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "loop-var-modified";

/// L0308: FOR loop control variable is modified inside the loop body.
struct LoopVarModified;

impl ErrorCode for LoopVarModified {
    fn code(&self) -> &'static str {
        "L0308"
    }

    fn description(&self) -> &'static str {
        "loop variable modified in body"
    }
}

/// Check whether an assignment target matches any active FOR loop control variable.
///
/// Called by the statement visitor for each assignment encountered.
pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var_access: VariableAccess<'db>,
    active_loop_vars: &[(VariableDecl<'db>, VariableAccess<'db>)],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if active_loop_vars.is_empty() {
        return;
    }

    let Some(assigned_decl) = resolve_var_decl(db, body, var_access) else {
        return;
    };

    for (control_decl, control_access) in active_loop_vars {
        if assigned_decl == *control_decl {
            let name = control_decl.get_name_ident(db).text(db);
            let mut d = diag()
                .message(format!(
                    "loop variable '{name}' is modified inside the loop body"
                ))
                .desc(&LoopVarModified)
                .range(var_access.get_span(db))
                .severity(DiagnosticSeverity::WARNING)
                .call();

            d.with_related(Related::new(
                format!("'{name}' is used here as control variable"),
                control_access.scope_id(db).file(db),
                control_access.get_span(db),
            ));

            diagnostics.push(d);
            break;
        }
    }
}

/// Resolve a control variable to its declaration, if possible.
pub fn resolve_control_var<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var_access: VariableAccess<'db>,
) -> Option<VariableDecl<'db>> {
    resolve_var_decl(db, body, var_access)
}

fn resolve_var_decl<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var_access: VariableAccess<'db>,
) -> Option<VariableDecl<'db>> {
    let VariableAccessKind::Symbolic(begin) = var_access.kind(db) else {
        return None;
    };
    let path_expr = begin.expr(db)?;
    match body.type_of_path_expr.get(&path_expr)? {
        Type::Variable((var_decl, _)) => Some(*var_decl),
        _ => None,
    }
}
