use std::sync::Arc;

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{BaseDatabase, file::File, tracked::get_ast},
};
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_semantic_index::Check,
        errors::sem_errors::{AnalysisError, ToIdeDiagnostic},
    },
    hir_def::semantic_index::semantic_index,
};

pub mod check_inheritance;
pub mod check_semantic_index;
pub mod errors;

#[salsa::tracked(no_eq)]
pub fn diagnostics_for_file(db: &dyn BaseDatabase, file: File) -> Arc<Vec<IdeDiagnostic>> {
    let mut all_diagnostics = vec![];

    let lexer_errors: Vec<AnalysisError> = get_ast::accumulated::<ParseErrorAccumulator>(db, file)
        .into_iter()
        .map(|e| (file, e).into())
        .collect::<Vec<_>>();
    let mut errors = vec![];
    semantic_index(db, file).collect_errors(db, &mut errors);

    all_diagnostics.extend(lexer_errors.into_iter().map(|e| e.to_diagnostic(db)));
    all_diagnostics.extend(errors.into_iter().map(|d| d.to_diagnostic(db)));

    Arc::new(all_diagnostics)
}
