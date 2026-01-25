use std::sync::Arc;

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{file::File, tracked::get_ast},
};
use db::{WorkspaceDataBase, configuration::Configuration};
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_duplicates::check_duplicate_pous,
        check_recursion::TypeDependencyGraph,
        errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    },
    hir_def::semantic_index::semantic_index,
};

use crate::{
    HirNodeInfo,
    check::check_duplicates::{
        check_duplicate_method_decls, check_duplicate_method_prots, check_duplicate_namespaces,
    },
    hir_def::{scope::ScopeId, semantic_index::SemanticIndex},
    hir_ty::{body_inference::infer_body_scope, signature::infer_signature},
};

pub mod check_duplicates;
pub mod check_recursion;
pub mod errors;

pub fn diagnostics_for_file(db: &dyn WorkspaceDataBase, file: File) -> Arc<Vec<IdeDiagnostic>> {
    let mut all_diagnostics = vec![];

    let lexer_errors: Vec<IdeDiagnostic> = get_ast::accumulated::<ParseErrorAccumulator>(db, file)
        .into_iter()
        .map(|e| AnalysisError::from((file, e)).to_diagnostic(db))
        .collect::<Vec<_>>();

    semantic_index(db, file).check(db, &mut all_diagnostics);

    all_diagnostics.extend(lexer_errors);

    Arc::new(all_diagnostics)
}

impl<'db> SemanticIndex<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        // Check if this file should contains program declarations
        let config = Configuration::try_get(db);

        TypeDependencyGraph::new(db, self).check_recursions(self, errors);

        // Get syntax errors
        self.errors
            .iter()
            .for_each(|err| errors.push(err.to_diagnostic(db)));

        self.global_pous.iter().for_each(|pou| {
            check_duplicate_pous(db, *pou, errors);
            pou.get_scope_id(db).check(db, errors);
        });

        // Namespaces
        self.namespaces.iter().for_each(|namespace| {
            check_duplicate_namespaces(db, *namespace)
                .values()
                .for_each(|diags| errors.extend_from_slice(diags));
            namespace.scope_id(db).check(db, errors)
        });

        // Programs
        self.programs.iter().for_each(|program| {
            program.get_scope_id(db).check(db, errors);
        });
    }
}

impl<'db> ScopeId<'db> {
    fn check(&self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        infer_signature(db, *self).errors.iter().for_each(|err| {
            errors.push(err.clone());
        });

        infer_body_scope(db, *self).errors.iter().for_each(|err| {
            errors.push(err.clone());
        });

        self.method_declarations(db).iter().for_each(|methods| {
            check_duplicate_method_decls(db, methods, errors);
            methods.iter().for_each(|method| {
                method.get_scope_id(db).check(db, errors);
            });
        });

        self.method_prototypes(db).iter().for_each(|methods| {
            check_duplicate_method_prots(db, methods, errors);
        });

        self.pous(db).iter().for_each(|pous| {
            pous.iter().for_each(|pou| {
                pou.get_scope_id(db).check(db, errors);
            });
        });
    }
}
