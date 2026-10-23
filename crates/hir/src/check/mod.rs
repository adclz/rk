// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use auto_lsp::default::db::file::File;
use db::{WorkspaceDataBase, workspace::Workspace};
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_duplicates::{
            check_config_fragment_collisions, check_duplicate_pous, check_duplicate_programs,
            check_single_configuration, check_single_resource,
        },
        check_layout::{TypeDependencyGraph, check_frames, check_storage},
        errors::{ToIdeDiagnostic, e00_syntax::SyntaxError},
    },
    hir_def::semantic_index::semantic_index,
    hir_ty::{
        calls::CallGraph, config::infer_config_result, head::init_inference::infer_initialization,
    },
};

use crate::check::errors::e14_config::ConfigError;
use crate::{
    HirNodeInfo,
    check::check_duplicates::check_duplicate_namespaces,
    hir_def::{scope::ScopeId, semantic_index::SemanticIndex},
    hir_ty::{body::infer_body, head::signature::infer_signature},
};

pub mod check_duplicates;
pub mod check_layout;
pub mod errors;
pub mod wasm_instructions;

#[salsa::tracked(returns(ref))]
pub fn diagnostics_for_file(db: &dyn WorkspaceDataBase, file: File) -> Arc<Vec<IdeDiagnostic>> {
    let mut all_diagnostics = vec![];

    // No config.toml: still analyze — defaults apply and the library
    // resolves — but lead with a hint that this file is outside a project.
    // (Analysis used to be skipped entirely here, which made a configless
    // `rk check` return nothing but the hint.)
    if let Some(config) = Workspace::try_get(db)
        && config.config_file(db).is_none()
    {
        all_diagnostics.push(ConfigError::NoConfigFileFound { file }.to_diagnostic(db, file));
    }

    // tree-sitter's errors, the nodes the builder had no place for, then why
    // no AST was built at all: the order auto-lsp accumulated them in.
    let parsed = db::syntax::parse(db, file);
    let lexer_errors: Vec<IdeDiagnostic> = parsed
        .syntax_errors
        .iter()
        .chain(db::syntax::unbuilt_nodes(db, file))
        .chain(&parsed.failure)
        .map(|e| SyntaxError::from_parse_error(db, file, e).to_diagnostic(db, file))
        .chain(parsed.unknown_pragmas.iter().map(|pragma| {
            errors::e15_pragma::PragmaError::UnknownPragma {
                name: compact_str::CompactString::from(pragma.name.as_str()),
                span: pragma.range,
            }
            .to_diagnostic(db, file)
        }))
        .collect::<Vec<_>>();

    semantic_index(db, file).check(db, &mut all_diagnostics);

    all_diagnostics.extend(lexer_errors);

    // In the order of the source, whatever found each: a syntax error is
    // not read after the errors of the lines below it, and two reports of
    // one construct stand together. Stable, so that what one pass reports
    // at one place keeps its order.
    all_diagnostics.sort_by_key(|d| (d.diagnostic.range.start, d.diagnostic.range.end));

    Arc::new(all_diagnostics)
}

/// Every diagnostic of `file`: what [`diagnostics_for_file`] finds, and the
/// frames of the recursive calls the file decides, in the order of the
/// source. `graph` is one the file's roots reach
/// ([`CallGraph::of_files`]).
pub fn check_file<'db>(
    db: &'db dyn WorkspaceDataBase,
    graph: &CallGraph<'db>,
    file: File,
) -> Vec<IdeDiagnostic> {
    let mut all = diagnostics_for_file(db, file).as_ref().clone();
    all.extend(frame_diagnostics(db, graph, file));
    all.sort_by_key(|d| (d.diagnostic.range.start, d.diagnostic.range.end));
    all
}

/// The frames of the recursive calls `file` decides: E1430 for one larger
/// than the stack, E0322 for one past what a module addresses. Which bodies
/// may call themselves is the call graph's, which crosses files, so these
/// are no part of [`diagnostics_for_file`]: a pass builds the graph once
/// and asks it for each file.
pub fn frame_diagnostics<'db>(
    db: &'db dyn WorkspaceDataBase,
    graph: &CallGraph<'db>,
    file: File,
) -> Vec<IdeDiagnostic> {
    let index = semantic_index(db, file);
    let mut errors = Vec::new();
    let pous = index.global_pous.iter().copied().chain(
        index
            .namespaces
            .iter()
            .flat_map(|ns| ns.pous(db).iter().copied()),
    );
    for pou in pous {
        let scope = pou.get_scope_id(db);
        check_frames(db, graph, scope, &mut errors);
        if let Some(methods) = scope.method_declarations(db) {
            for method in methods {
                check_frames(db, graph, method.get_scope_id(db), &mut errors);
            }
        }
    }
    for program in index.programs.iter() {
        check_frames(db, graph, program.get_scope_id(db), &mut errors);
    }
    for config in index.configs.iter() {
        check_frames(db, graph, config.get_scope_id(db), &mut errors);
    }
    errors
}

impl<'db> SemanticIndex<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        TypeDependencyGraph::new(db, self).check_recursions(self, errors);

        // Get syntax errors
        self.errors.iter().for_each(|err| errors.push(err.clone()));

        self.scope.check(db, errors);

        // Programs
        self.programs.iter().for_each(|program| {
            check_duplicate_programs(db, *program, errors);
            program.get_scope_id(db).check(db, errors);
        });

        // Configurations — validate program type references and task references.
        self.configs.iter().for_each(|config| {
            check_config_fragment_collisions(db, *config, errors);
            check_single_configuration(db, *config, errors);
            check_single_resource(db, *config, errors);
            errors.extend(infer_config_result(db, *config).errors.iter().cloned());
            config.get_scope_id(db).check(db, errors);
        });
    }
}

impl<'db> ScopeId<'db> {
    fn check(&self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        // Runs both inference queries and collects errors

        infer_signature(db, *self).errors.iter().for_each(|err| {
            errors.push(err.clone());
        });

        infer_initialization(db, *self)
            .errors
            .iter()
            .for_each(|err| {
                errors.push(err.clone());
            });
        check_storage(db, *self, errors);

        infer_body(db, *self).errors.iter().for_each(|err| {
            errors.push(err.clone());
        });

        // Methods and nested POUs are not inferred by the result of scope inference
        // so we need to treat them separately by calling check again on their scopes

        if let Some(namespaces) = self.namespaces(db) {
            namespaces
                .iter()
                .filter(|ns| {
                    // The GLOBAL scope's list is the file's FLAT namespace
                    // index (nested ones included — name resolution wants
                    // them all); the recursion below reaches the nested ones
                    // through their parents. Checking the flat list whole
                    // visited every nested namespace twice, and every
                    // diagnostic in one came out doubled.
                    if !self.is_global(db) {
                        return true;
                    }
                    use crate::hir_def::scope::ScopeKind;
                    use crate::hir_def::semantic_index::get_scope;
                    match get_scope(db, ns.scope_id(db)).parent {
                        Some(parent) => {
                            matches!(get_scope(db, parent).kind, ScopeKind::Global)
                        }
                        None => true,
                    }
                })
                .for_each(|ns| {
                    check_duplicate_namespaces(db, *ns, errors);
                    ns.scope_id(db).check(db, errors);
                });
        }

        if let Some(pous) = self.pous(db) {
            pous.iter().for_each(|pou| {
                if self.is_global(db) {
                    check_duplicate_pous(db, *pou, errors);
                }
                pou.get_scope_id(db).check(db, errors);
            });
        }

        if let Some(methods) = self.method_declarations(db) {
            methods.iter().for_each(|method| {
                method.get_scope_id(db).check(db, errors);
            });
        }

        self.method_prototypes(db).iter().for_each(|methods| {
            methods.iter().for_each(|method| {
                method.get_scope_id(db).check(db, errors);
            });
        });
    }
}
