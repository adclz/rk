use std::sync::Arc;

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{file::File, tracked::get_ast},
};
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_duplicates::check_duplicate_pous,
        check_recursion::TypeDependencyGraph,
        check_semantic_index::Check,
        errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    },
    hir_def::semantic_index::semantic_index,
};

pub mod check_duplicates;
pub mod check_inheritance;
pub mod check_recursion;
pub mod check_scope;
pub mod check_semantic_index;
pub mod check_using;
pub mod errors;

pub fn diagnostics_for_file(db: &dyn WorkspaceDataBase, file: File) -> Arc<Vec<IdeDiagnostic>> {
    let mut all_diagnostics = vec![];

    let lexer_errors: Vec<AnalysisError> = get_ast::accumulated::<ParseErrorAccumulator>(db, file)
        .into_iter()
        .map(|e| (file, e).into())
        .collect::<Vec<_>>();
    let mut errors = vec![];
    let semantic_index = semantic_index(db, file);
    semantic_index.check(db, &mut errors);

    let mut recursion_errors = TypeDependencyGraph::new(db, semantic_index);
    let recursion_errors = recursion_errors.find_recursion(semantic_index);

    all_diagnostics.extend(check_duplicate_pous(db, file));
    all_diagnostics.extend(lexer_errors.into_iter().map(|e| e.to_diagnostic(db)));
    all_diagnostics.extend(recursion_errors.into_iter().map(|e| e.to_diagnostic(db)));
    all_diagnostics.extend(errors);

    Arc::new(all_diagnostics)
}
