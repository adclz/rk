use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        pous::variable::{VariableDecl, VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::body::infer_body,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

/// W0101: variable is declared but never used in the body.
struct UnusedVariable;

impl ErrorCode for UnusedVariable {
    fn code(&self) -> &'static str {
        "W0101"
    }

    fn description(&self) -> &'static str {
        "unused code"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Only scopes with bodies can have unused variables
    let has_body = matches!(
        get_scope(db, scope).kind,
        ScopeKind::Pou(hir::hir_def::pous::pou::Pou::Function(_))
            | ScopeKind::Pou(hir::hir_def::pous::pou::Pou::FunctionBlock(_))
            | ScopeKind::MethodDecl(_)
            | ScopeKind::Program(_)
    );
    if !has_body {
        return;
    }

    let body = infer_body(db, scope);
    let def_map = scope.def_map(db);

    for (_, var) in &def_map.global_variables {
        check_variable(db, scope, *var, &body.variables_used, diagnostics);
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
    if name.as_str() == "_" {
        return;
    }

    let mut diag  = diag()
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
