use auto_lsp::default::db::file::File;
use db::{WorkspaceDataBase, config_file::LinterConfig};
use hir::{
    HirNodeInfo,
    hir_def::{
        namespace::NamespaceDecl,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
        using::Using,
    },
    hir_ty::body::infer_body,
};
use ide_diagnostic::IdeDiagnostic;

pub mod case_without_else;
pub mod constant_condition;
pub mod dead_code;
pub mod duplicate_var_section;
pub mod effectless_statement;
pub mod for_loop_step_sign;
pub mod input_assignment;
pub mod self_assignment;
pub mod shadowing_variable;
pub mod unnecessary_else;
pub mod uninitialized_output;
pub mod unused_import;
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

    // Collect all body scopes, all signature scopes, and usings for file-level lints
    let mut body_scopes = vec![];
    let mut all_scopes: Vec<ScopeId> = vec![];
    let mut all_usings: Vec<Using> = vec![];

    // Global usings
    all_usings.extend_from_slice(sema.scope.usings(db));

    lint_scope(db, config, sema.scope, &mut body_scopes, diagnostics);

    for pou in sema.global_pous.iter() {
        let scope = pou.get_scope_id(db);
        all_usings.extend_from_slice(scope.usings(db));
        all_scopes.push(scope);
        lint_scope(db, config, scope, &mut body_scopes, diagnostics);

        if let Some(methods) = scope.method_declarations(db) {
            for method in methods {
                let method_scope = method.get_scope_id(db);
                all_usings.extend_from_slice(method_scope.usings(db));
                all_scopes.push(method_scope);
                lint_scope(db, config, method_scope, &mut body_scopes, diagnostics);
            }
        }
    }

    for ns in sema.namespaces.iter() {
        collect_namespace_scopes(
            db,
            config,
            *ns,
            &mut all_scopes,
            &mut body_scopes,
            &mut all_usings,
            diagnostics,
        );
    }

    for program in sema.programs.iter() {
        let scope = program.get_scope_id(db);
        all_usings.extend_from_slice(scope.usings(db));
        all_scopes.push(scope);
        lint_scope(db, config, scope, &mut body_scopes, diagnostics);
    }

    // File-level lint: unused imports (needs all scopes collected)
    if config.is_enabled(unused_import::NAME) {
        unused_import::check(db, &all_scopes, &body_scopes, &all_usings, diagnostics);
    }
}

/// Recursively collect scopes and usings from namespace declarations and their POUs.
fn collect_namespace_scopes<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &LinterConfig,
    ns: NamespaceDecl<'db>,
    all_scopes: &mut Vec<ScopeId<'db>>,
    body_scopes: &mut Vec<ScopeId<'db>>,
    all_usings: &mut Vec<Using<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for pou in ns.pous(db) {
        let scope = pou.get_scope_id(db);
        all_usings.extend_from_slice(scope.usings(db));
        all_scopes.push(scope);
        lint_scope(db, config, scope, body_scopes, diagnostics);

        if let Some(methods) = scope.method_declarations(db) {
            for method in methods {
                let method_scope = method.get_scope_id(db);
                all_usings.extend_from_slice(method_scope.usings(db));
                all_scopes.push(method_scope);
                lint_scope(db, config, method_scope, body_scopes, diagnostics);
            }
        }
    }

    for nested in ns.namespaces(db) {
        collect_namespace_scopes(
            db,
            config,
            *nested,
            all_scopes,
            body_scopes,
            all_usings,
            diagnostics,
        );
    }
}

/// Run all scope-level lint rules against a single scope.
fn lint_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &LinterConfig,
    scope: ScopeId<'db>,
    body_scopes: &mut Vec<ScopeId<'db>>,
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

    body_scopes.push(scope);

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
    if config.is_enabled(input_assignment::NAME) {
        input_assignment::check(db, scope, body, diagnostics);
    }
    if config.is_enabled(self_assignment::NAME) {
        self_assignment::check(db, scope, body, diagnostics);
    }
    if config.is_enabled(constant_condition::NAME) {
        constant_condition::check(db, scope, diagnostics);
    }
    if config.is_enabled(uninitialized_output::NAME) {
        uninitialized_output::check(db, scope, body, diagnostics);
    }
    if config.is_enabled(unnecessary_else::NAME) {
        unnecessary_else::check(db, scope, diagnostics);
    }
}
