use std::sync::Arc;

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{BaseDatabase, file::File, tracked::get_ast},
};
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_global_pous::check_duplicate_pous,
        check_semantic_index::Check,
        errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    },
    hir_def::semantic_index::semantic_index,
};

pub mod check_array;
pub mod check_enum;
pub mod check_global_pous;
pub mod check_inheritance;
pub mod check_namespaces;
pub mod check_scope;
pub mod check_semantic_index;
pub mod check_struct;
pub mod check_subrange;
pub mod check_using;
pub mod check_variables;
pub mod errors;

pub fn diagnostics_for_file(db: &dyn WorkspaceDataBase, file: File) -> Arc<Vec<IdeDiagnostic>> {
    let mut all_diagnostics = vec![];

    let lexer_errors: Vec<AnalysisError> = get_ast::accumulated::<ParseErrorAccumulator>(db, file)
        .into_iter()
        .map(|e| (file, e).into())
        .collect::<Vec<_>>();
    let mut errors = vec![];
    semantic_index(db, file).check(db, &mut errors);

    all_diagnostics.extend(check_duplicate_pous(db, file));
    all_diagnostics.extend(lexer_errors.into_iter().map(|e| e.to_diagnostic(db)));
    all_diagnostics.extend(errors);

    Arc::new(all_diagnostics)
}
