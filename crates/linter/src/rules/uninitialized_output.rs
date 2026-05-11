use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName,
    hir_def::{
        expressions::expression::VariableAccessKind,
        pous::variable::{VariableDecl, VariableKind},
        scope::ScopeId,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "uninitialized-output";

/// L0105: one or more VAR_OUTPUT variables are never assigned in the body.
///
/// All uninitialized outputs for a given POU body are collapsed into a single
/// diagnostic so the user gets one summary instead of N pointers. Per other toolchains
/// VAR_OUTPUT semantics, leaving an output unassigned is permitted for
/// FUNCTION_BLOCK / PROGRAM (compiler zero-inits the instance field) but
/// surfacing it as an INFO-level lint helps catch unintended omissions.
struct UninitializedOutput;

impl ErrorCode for UninitializedOutput {
    fn code(&self) -> &'static str {
        "L0205"
    }

    fn description(&self) -> &'static str {
        "uninitialized output"
    }
}

/// Collect the variable assigned by a single assignment LHS.
/// Called by the unified visitor for each assignment statement.
pub fn collect_assigned<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var: hir::hir_def::expressions::expression::VariableAccess<'db>,
    assigned: &mut FxHashSet<VariableDecl<'db>>,
) {
    let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
        return;
    };
    let Some(path_expr) = begin.expr(db) else {
        return;
    };
    if let Some(&Type::Variable((var_decl, _))) = body.type_of_path_expr.get(&path_expr) {
        assigned.insert(var_decl);
    }
}

/// Check VAR_OUTPUT variables against the set of assigned variables.
/// Called after the unified visitor has finished collecting.
pub fn check_outputs<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    assigned: &FxHashSet<VariableDecl<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let def_map = scope.def_map(db);
    let missing: Vec<_> = def_map
        .global_variables
        .values()
        .filter(|var| var.kind(db) == VariableKind::Output)
        .filter(|var| var.init(db).is_none())
        .filter(|var| !assigned.contains(*var))
        .copied()
        .collect();

    if missing.is_empty() {
        return;
    }

    let names: Vec<_> = missing
        .iter()
        .map(|v| format!("'{}'", v.name(db).text(db)))
        .collect();

    // Anchor the diagnostic on the first uninitialized output's name span.
    // The related-info entries cover the rest of the declarations.
    let anchor = missing[0].get_name_span(db);

    let mut d = diag()
        .message(format!(
            "{} VAR_OUTPUT {} never assigned in the body: {}",
            missing.len(),
            if missing.len() > 1 { "are" } else { "is" },
            names.join(", "),
        ))
        .desc(&UninitializedOutput)
        .range(anchor)
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    for var in &missing {
        d.with_related(Related::new(
            format!("'{}' declared here", var.name(db).text(db)),
            var.scope_id(db).file(db),
            var.get_name_span(db),
        ));
    }

    diagnostics.push(d);
}
