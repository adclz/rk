use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::expression::VariableAccessKind,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "missing-return";

/// L0316: function or method with a return type never assigns the return value.
struct MissingReturn;

impl ErrorCode for MissingReturn {
    fn code(&self) -> &'static str {
        "L0316"
    }

    fn description(&self) -> &'static str {
        "missing return assignment"
    }
}

/// Called by the stmt_visitor for each assignment — check if LHS is the return
/// variable by verifying the inferred type is the function/method itself.
pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var: hir::hir_def::expressions::expression::VariableAccess<'db>,
    scope: ScopeId<'db>,
) -> bool {
    let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
        return false;
    };
    let Some(path_expr) = begin.expr(db) else {
        return false;
    };
    let Some(ty) = body.type_of_path_expr.get(&path_expr) else {
        return false;
    };

    // Check if the inferred type is the function/method that owns this scope
    match ty {
        Type::Function(f) => f.get_scope_id(db) == scope,
        Type::MethodDecl(m) => m.get_scope_id(db) == scope,
        _ => false,
    }
}

/// Post-walk check: if no return assignment was found, emit the diagnostic.
pub fn check_result<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    return_assigned: bool,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if return_assigned {
        return;
    }

    let scope_data = get_scope(db, scope);

    let (pou_name, pou_kind, name_span) = match scope_data.kind {
        ScopeKind::Pou(Pou::Function(f)) => {
            if f.return_type(db).is_none() {
                return;
            }
            (f.name(db).text(db), "FUNCTION", f.get_name_span(db))
        }
        ScopeKind::MethodDecl(m) => {
            if m.return_type(db).is_none() {
                return;
            }
            (m.name(db).text(db), "METHOD", m.get_name_span(db))
        }
        _ => return,
    };

    diagnostics.push(
        diag()
            .message(format!(
                "{pou_kind} '{pou_name}' has a return type but never assigns a return value"
            ))
            .desc(&MissingReturn)
            .range(hir::denormalize(db, scope.file(db), &name_span).unwrap_or_default())
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}
