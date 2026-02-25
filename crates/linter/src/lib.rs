use std::sync::Arc;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    check::diagnostics_for_file,
    hir_def::{scope::ScopeId, semantic_index::semantic_index},
};
use ide_diagnostic::IdeDiagnostic;

pub mod rules;

/// Run all HIR diagnostics **and** lint rules for a file.
///
/// This wraps [`diagnostics_for_file`] and appends linter warnings.
/// Callers (server, CLI) should use this instead of `diagnostics_for_file` directly.
pub fn lint_and_check_file(db: &dyn WorkspaceDataBase, file: File) -> Arc<Vec<IdeDiagnostic>> {
    let mut diagnostics = diagnostics_for_file(db, file).as_ref().clone();
    lint_file(db, file, &mut diagnostics);
    Arc::new(diagnostics)
}

/// Run all lint rules on a file's scopes, appending warnings to `diagnostics`.
pub fn lint_file(
    db: &dyn WorkspaceDataBase,
    file: File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let sema = semantic_index(db, file);

    // Lint the global scope
    lint_scope(db, sema.scope, diagnostics);

    // Lint each POU
    for pou in &sema.global_pous {
        let scope = pou.get_scope_id(db);
        lint_scope(db, scope, diagnostics);

        // Lint nested methods
        if let Some(methods) = scope.method_declarations(db) {
            for method in methods {
                lint_scope(db, method.get_scope_id(db), diagnostics);
            }
        }
    }

    // Lint programs
    for program in &sema.programs {
        lint_scope(db, program.get_scope_id(db), diagnostics);
    }
}

/// Run all lint rules against a single scope.
fn lint_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    rules::unused_variable::check(db, scope, diagnostics);
    rules::shadowing_variable::check(db, scope, diagnostics);
}
