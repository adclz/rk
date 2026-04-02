use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        pous::variable::{VariableDecl, VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::body::{BodyInferenceResult, infer_body},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "unused-variable";

/// L0101: variable is declared but never used in the body.
struct UnusedVariable;

impl ErrorCode for UnusedVariable {
    fn code(&self) -> &'static str {
        "L0101"
    }

    fn description(&self) -> &'static str {
        "unused code"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let def_map = scope.def_map(db);

    // Collect variables used in the body itself
    let mut all_used = body.variables_used.clone();

    // For FB/class scopes, also collect variables used by child methods via THIS
    if let Some(methods) = scope.method_declarations(db) {
        for method in methods {
            let method_scope = method.get_scope_id(db);
            let method_body = infer_body(db, method_scope);
            all_used.extend(method_body.variables_used.iter().copied());
        }
    }

    for var in def_map.global_variables.values() {
        check_variable(db, scope, *var, &all_used, diagnostics);
    }
}

fn check_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    var: VariableDecl<'db>,
    variables_used: &rustc_hash::FxHashSet<VariableDecl<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Skip variable kinds that are inherently "used" externally
    match var.kind(db) {
        // Outputs are read by callers
        VariableKind::Output => return,
        // InOut is both read and written by callers
        VariableKind::InOut => return,
        // Global/External variables are shared across POUs
        VariableKind::Global | VariableKind::External => return,
        // Config variables are set by configuration
        VariableKind::Config => return,
        // Access variables are for communication paths
        VariableKind::Access => return,
        // Var, Input, Temp — check these
        _ => {}
    }

    // Skip inputs on PROGRAM — they are assigned by CONFIGURATION
    if matches!(var.kind(db), VariableKind::Input)
        && matches!(get_scope(db, scope).kind, ScopeKind::Program(_))
    {
        return;
    }

    if variables_used.contains(&var) {
        return;
    }

    let name = var.get_name_ident(db).text(db);

    // Skip conventional "don't care" names
    if name.as_str().starts_with("_") {
        return;
    }

    let mut diag = diag()
        .message(format!("unused variable '{name}'"))
        .severity(DiagnosticSeverity::WARNING)
        .tags(vec![DiagnosticTag::UNNECESSARY])
        .desc(&UnusedVariable)
        .range(var.get_span(db))
        .call();

    diag.with_note(format!(
        "if this is intentional, prefix it with an underscore:\n'_{name}'"
    ));

    diagnostics.push(diag);
}
