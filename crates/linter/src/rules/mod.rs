use auto_lsp::default::db::file::File;
use db::{
    WorkspaceDataBase,
    config_file::{LinterConfig, Select},
};
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

pub mod bool_comparison;
pub mod case_without_else;
pub mod collapsible_if;
pub mod constant_condition;
pub mod constant_loop_bounds;
pub mod dead_code;
pub mod default_for_step;
pub mod division_by_zero;
pub mod duplicate_case;
pub mod duplicate_configuration;
pub mod duplicate_namespace;
pub mod duplicate_var_section;
pub mod effectless_statement;
pub mod empty_body;
pub mod empty_case_branch;
pub mod empty_if_branch;
pub mod empty_loop_body;
pub mod empty_type;
pub mod external_mutation;
pub mod for_loop_step_sign;
pub mod global_without_external;
pub mod identical_sub_expr;
pub mod identity_operation;
pub mod input_assignment;
pub mod invalid_pragma;
pub mod loop_var_modified;
pub mod method_shadows_member;
pub mod missing_input_param;
pub mod missing_return;
pub mod negated_comparison;
pub mod negated_condition;
pub mod once_violation;
pub mod redundant_not;
pub mod self_assignment;
pub mod self_comparison;
pub mod self_shadowing;
pub mod shadowing_variable;
pub mod single_element_array;
pub mod stmt_visitor;
pub mod sub_self;
pub mod uninitialized_output;
pub mod unnecessary_else;
pub mod unnecessary_parens;
pub mod unused_import;
pub mod unused_return_type;
pub mod unused_variable;
pub mod warn_pragma;
pub mod yoda_condition;

/// The rules `Select::Recommended` turns on: the ones that report a probable
/// BUG rather than a matter of taste. They are exactly the rules that emit at
/// warning severity — style (hints) and declaration notes (info) stay opt-in,
/// so a workspace that has said nothing about linting is not buried in taste.
pub const RECOMMENDED_RULE_NAMES: &[&str] = &[
    warn_pragma::NAME,
    invalid_pragma::NAME,
    dead_code::NAME,
    for_loop_step_sign::NAME,
    input_assignment::NAME,
    constant_condition::NAME,
    division_by_zero::NAME,
    duplicate_case::NAME,
    loop_var_modified::NAME,
    self_assignment::NAME,
    self_comparison::NAME,
    identical_sub_expr::NAME,
    identity_operation::NAME,
    sub_self::NAME,
    constant_loop_bounds::NAME,
    self_shadowing::NAME,
    missing_return::NAME,
    external_mutation::NAME,
    method_shadows_member::NAME,
    global_without_external::NAME,
];

/// Whether `name` runs under `config`: an explicit entry in `[linter.rules]`
/// wins, otherwise the `select` baseline decides.
pub fn is_enabled(config: &LinterConfig, name: &str) -> bool {
    if let Some(explicit) = config.rule_override(name) {
        return explicit;
    }
    match config.select() {
        Select::All => true,
        Select::Recommended => RECOMMENDED_RULE_NAMES.contains(&name),
        Select::None => false,
    }
}

/// All lint rule names, for building configs that enable/disable specific rules.
pub const ALL_RULE_NAMES: &[&str] = &[
    bool_comparison::NAME,
    case_without_else::NAME,
    collapsible_if::NAME,
    constant_condition::NAME,
    constant_loop_bounds::NAME,
    dead_code::NAME,
    default_for_step::NAME,
    division_by_zero::NAME,
    duplicate_case::NAME,
    duplicate_configuration::NAME,
    duplicate_namespace::NAME,
    duplicate_var_section::NAME,
    effectless_statement::NAME,
    empty_body::NAME,
    empty_case_branch::NAME,
    external_mutation::NAME,
    empty_if_branch::NAME,
    empty_loop_body::NAME,
    empty_type::NAME,
    for_loop_step_sign::NAME,
    global_without_external::NAME,
    identical_sub_expr::NAME,
    identity_operation::NAME,
    input_assignment::NAME,
    invalid_pragma::NAME,
    loop_var_modified::NAME,
    method_shadows_member::NAME,
    missing_input_param::NAME,
    missing_return::NAME,
    negated_comparison::NAME,
    negated_condition::NAME,
    once_violation::NAME,
    redundant_not::NAME,
    self_assignment::NAME,
    self_comparison::NAME,
    self_shadowing::NAME,
    shadowing_variable::NAME,
    single_element_array::NAME,
    sub_self::NAME,
    uninitialized_output::NAME,
    unnecessary_else::NAME,
    unnecessary_parens::NAME,
    unused_import::NAME,
    unused_return_type::NAME,
    unused_variable::NAME,
    warn_pragma::NAME,
    yoda_condition::NAME,
];

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
    // File-level lints
    run_lint(duplicate_var_section::NAME, diagnostics, |d| {
        duplicate_var_section::check(db, file, config, d)
    });

    if is_enabled(config, duplicate_configuration::NAME) {
        run_lint(duplicate_configuration::NAME, diagnostics, |d| {
            duplicate_configuration::check(db, file, d)
        });
    }

    if is_enabled(config, duplicate_namespace::NAME) {
        run_lint(duplicate_namespace::NAME, diagnostics, |d| {
            duplicate_namespace::check(db, file, d)
        });
    }

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

        // DataType-level lints
        if is_enabled(config, empty_type::NAME)
            && let Pou::DataType(dt) = pou
        {
            run_lint(empty_type::NAME, diagnostics, |d| {
                empty_type::check(db, *dt, d)
            });
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
    if is_enabled(config, unused_import::NAME) {
        run_lint(unused_import::NAME, diagnostics, |d| {
            unused_import::check(db, &all_scopes, &body_scopes, &all_usings, d)
        });
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

/// Run a lint check and tag each new diagnostic with a note showing the rule name.
pub(crate) fn run_lint(
    name: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
    f: impl FnOnce(&mut Vec<IdeDiagnostic>),
) {
    let before = diagnostics.len();
    f(diagnostics);
    for d in &mut diagnostics[before..] {
        d.with_note(format!("lint rule: {}", name));
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

    if is_enabled(config, empty_body::NAME) {
        run_lint(empty_body::NAME, diagnostics, |d| {
            empty_body::check(db, scope, d)
        });
    }
    if is_enabled(config, invalid_pragma::NAME) {
        run_lint(invalid_pragma::NAME, diagnostics, |d| {
            invalid_pragma::check(db, scope, d)
        });
    }
    if is_enabled(config, self_shadowing::NAME) {
        run_lint(self_shadowing::NAME, diagnostics, |d| {
            self_shadowing::check(db, scope, d)
        });
    }
    if is_enabled(config, single_element_array::NAME) {
        let variables: &[hir::hir_def::pous::variable::VariableDecl] =
            match get_scope(db, scope).kind {
                ScopeKind::Pou(Pou::Function(f)) => f.variables(db),
                ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.variables(db),
                ScopeKind::Pou(Pou::Class(cl)) => cl.variables(db),
                ScopeKind::MethodDecl(m) => m.variables(db),
                ScopeKind::Program(program) => program.variables(db),
                _ => &[],
            };
        run_lint(single_element_array::NAME, diagnostics, |d| {
            for var in variables {
                single_element_array::check_spec(db, var.spec(db).kind(db), d);
            }
        });
    }

    let body = infer_body(db, scope);

    if is_enabled(config, unused_variable::NAME) {
        run_lint(unused_variable::NAME, diagnostics, |d| {
            unused_variable::check(db, scope, body, d)
        });
    }
    if is_enabled(config, shadowing_variable::NAME) {
        run_lint(shadowing_variable::NAME, diagnostics, |d| {
            shadowing_variable::check(db, body, d)
        });
    }
    if is_enabled(config, method_shadows_member::NAME) {
        run_lint(method_shadows_member::NAME, diagnostics, |d| {
            method_shadows_member::check(db, body, d)
        });
    }
    if is_enabled(config, global_without_external::NAME) {
        run_lint(global_without_external::NAME, diagnostics, |d| {
            global_without_external::check(db, body, d)
        });
    }
    if is_enabled(config, unused_return_type::NAME) {
        run_lint(unused_return_type::NAME, diagnostics, |d| {
            unused_return_type::check(db, body, d)
        });
    }
    if is_enabled(config, effectless_statement::NAME) {
        run_lint(effectless_statement::NAME, diagnostics, |d| {
            effectless_statement::check(db, body, d)
        });
    }
    if is_enabled(config, case_without_else::NAME) {
        run_lint(case_without_else::NAME, diagnostics, |d| {
            case_without_else::check(db, body, d)
        });
    }
    if is_enabled(config, division_by_zero::NAME) {
        run_lint(division_by_zero::NAME, diagnostics, |d| {
            division_by_zero::check(db, scope, d)
        });
    }
    if is_enabled(config, dead_code::NAME) {
        run_lint(dead_code::NAME, diagnostics, |d| {
            dead_code::check(db, body, d)
        });
    }
    if is_enabled(config, for_loop_step_sign::NAME) {
        run_lint(for_loop_step_sign::NAME, diagnostics, |d| {
            for_loop_step_sign::check(db, body, d)
        });
    }
    // Statement-walking lints: single pass over the statement tree
    stmt_visitor::check(db, config, scope, body, diagnostics);

    if is_enabled(config, warn_pragma::NAME) {
        run_lint(warn_pragma::NAME, diagnostics, |d| {
            warn_pragma::check(db, body, d)
        });
    }
    if is_enabled(config, once_violation::NAME) {
        run_lint(once_violation::NAME, diagnostics, |d| {
            once_violation::check(db, body, d)
        });
    }
}

#[cfg(test)]
mod select_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn config(select: Option<Select>, rules: &[(&str, bool)]) -> LinterConfig {
        LinterConfig {
            select,
            rules: (!rules.is_empty()).then(|| {
                rules
                    .iter()
                    .map(|(n, v)| ((*n).to_string(), *v))
                    .collect::<BTreeMap<_, _>>()
            }),
        }
    }

    #[test]
    fn the_default_is_recommended_not_silence() {
        // The whole point of the change: a config that says nothing still lints.
        let c = LinterConfig::default();
        assert!(is_enabled(&c, self_assignment::NAME));
        assert!(!is_enabled(&c, yoda_condition::NAME));
    }

    #[test]
    fn select_picks_the_baseline() {
        let all = config(Some(Select::All), &[]);
        let none = config(Some(Select::None), &[]);
        assert!(is_enabled(&all, yoda_condition::NAME));
        assert!(!is_enabled(&none, self_assignment::NAME));
        for name in ALL_RULE_NAMES {
            assert!(is_enabled(&all, name), "{name} must run under `all`");
            assert!(!is_enabled(&none, name), "{name} must not run under `none`");
        }
    }

    #[test]
    fn an_explicit_rule_beats_the_baseline_both_ways() {
        let opt_in = config(Some(Select::None), &[(yoda_condition::NAME, true)]);
        assert!(is_enabled(&opt_in, yoda_condition::NAME));
        assert!(!is_enabled(&opt_in, self_assignment::NAME));

        let opt_out = config(None, &[(self_assignment::NAME, false)]);
        assert!(!is_enabled(&opt_out, self_assignment::NAME));
        assert!(is_enabled(&opt_out, dead_code::NAME));
    }

    #[test]
    fn every_recommended_rule_is_a_real_rule() {
        // A typo here would silently drop a rule from the default set.
        for name in RECOMMENDED_RULE_NAMES {
            assert!(
                ALL_RULE_NAMES.contains(name),
                "{name} is not a known rule name"
            );
        }
        assert_eq!(RECOMMENDED_RULE_NAMES.len(), 20);
    }
}
