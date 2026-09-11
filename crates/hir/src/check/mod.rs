use std::sync::Arc;

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{file::File, tracked::get_ast},
};
use db::{WorkspaceDataBase, workspace::Workspace};
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_duplicates::{
            check_config_fragment_collisions, check_duplicate_pous, check_duplicate_programs,
            check_single_configuration, check_single_resource,
        },
        check_recursion::TypeDependencyGraph,
        errors::{ToIdeDiagnostic, e00_syntax::SyntaxError},
    },
    hir_def::semantic_index::semantic_index,
    hir_ty::{config::infer_config_result, head::init_inference::infer_initialization},
};

use crate::check::errors::e14_config::ConfigError;
use crate::{
    HirNodeInfo,
    check::check_duplicates::check_duplicate_namespaces,
    hir_def::{scope::ScopeId, semantic_index::SemanticIndex},
    hir_ty::{body::infer_body, head::signature::infer_signature},
};

pub mod check_duplicates;
pub mod check_recursion;
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

    let lexer_errors: Vec<IdeDiagnostic> = get_ast::accumulated::<ParseErrorAccumulator>(db, file)
        .into_iter()
        .map(|e| SyntaxError::from_parse_error(db, file, e).to_diagnostic(db, file))
        .collect::<Vec<_>>();

    semantic_index(db, file).check(db, &mut all_diagnostics);

    all_diagnostics.extend(lexer_errors);

    Arc::new(all_diagnostics)
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
