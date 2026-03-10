use auto_lsp::default::db::file::File;
use db::{WorkspaceDataBase, config_file::LinterConfig};
use hir::{
    HirNodeInfo,
    hir_def::{
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::body::infer_body,
};
use ide_diagnostic::IdeDiagnostic;

pub mod case_without_else;
pub mod dead_code;
pub mod duplicate_var_section;
pub mod effectless_statement;
pub mod for_loop_step_sign;
pub mod shadowing_variable;
pub mod unused_return_type;
pub mod unused_variable;

/// Run all lint rules on a file, appending warnings to `diagnostics`.
///
/// The caller (LSP, CLI) is responsible for checking the config and deciding
/// whether to call this at all (i.e. whether a `[linter]` section exists).
pub fn lint_file(
    db: &dyn WorkspaceDataBase,
    file: File,
    config: &LinterConfig,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // File-level lints (tree-sitter based, run once per file)
    duplicate_var_section::check(db, file, config, diagnostics);

    // Scope-level lints (HIR based)
    let sema = semantic_index(db, file);

    lint_scope(db, config, sema.scope, diagnostics);

    for pou in sema.global_pous.iter() {
        let scope = pou.get_scope_id(db);
        lint_scope(db, config, scope, diagnostics);

        if let Some(methods) = scope.method_declarations(db) {
            for method in methods {
                lint_scope(db, config, method.get_scope_id(db), diagnostics);
            }
        }
    }

    for program in sema.programs.iter() {
        lint_scope(db, config, program.get_scope_id(db), diagnostics);
    }
}

/// Run all scope-level lint rules against a single scope.
fn lint_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &LinterConfig,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let has_body = matches!(
        get_scope(db, scope).kind,
        ScopeKind::Pou(Pou::Function(_))
            | ScopeKind::Pou(Pou::FunctionBlock(_))
            | ScopeKind::MethodDecl(_)
            | ScopeKind::Program(_)
    );
    if !has_body {
        return;
    }

    let body = infer_body(db, scope);

    if config.is_enabled(unused_variable::NAME) {
        unused_variable::check(db, scope, body, diagnostics);
    }
    if config.is_enabled(shadowing_variable::NAME) {
        shadowing_variable::check(db, body, diagnostics);
    }
    if config.is_enabled(unused_return_type::NAME) {
        unused_return_type::check(db, body, diagnostics);
    }
    if config.is_enabled(effectless_statement::NAME) {
        effectless_statement::check(db, body, diagnostics);
    }
    if config.is_enabled(case_without_else::NAME) {
        case_without_else::check(db, body, diagnostics);
    }
    if config.is_enabled(dead_code::NAME) {
        dead_code::check(db, body, diagnostics);
    }
    if config.is_enabled(for_loop_step_sign::NAME) {
        for_loop_step_sign::check(db, body, diagnostics);
    }
}
