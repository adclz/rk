use std::sync::Arc;

use auto_lsp::{
    core::errors::{LexerError, ParseError, ParseErrorAccumulator},
    default::db::{BaseDatabase, file::File, tracked::get_ast},
};
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_semantic_index::Check,
        errors::sem_errors::{AnalysisError, SyntaxError, ToIdeDiagnostic},
    },
    def::semantic_index::semantic_index,
};

pub mod check_semantic_index;
pub mod check_inheritance;
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

impl<'db> From<(File, &ParseErrorAccumulator)> for AnalysisError<'db> {
    fn from((file, err): (File, &ParseErrorAccumulator)) -> Self {
        match &err.0 {
            ParseError::LexerError { span, error } => match error {
                LexerError::Missing {
                    range,
                    error,
                    grammar_name,
                } => AnalysisError::SyntaxError(SyntaxError::MissingNode {
                    file,
                    span: range.into(),
                    err: error.to_owned(),
                    grammar_name,
                }),
                LexerError::Syntax {
                    range,
                    error,
                    affected,
                } => AnalysisError::SyntaxError(SyntaxError::SyntaxError {
                    span: range.into(),
                    err: error.to_owned(),
                }),
            },
            _ => unreachable!("Only lexer errors should be present here"),
        }
    }
}
