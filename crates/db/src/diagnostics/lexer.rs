
use std::collections::HashMap;

use auto_lsp::core::errors::{LexerError, ParseError, ParseErrorAccumulator};
use auto_lsp::lsp_types::{DiagnosticRelatedInformation, WorkspaceEdit};
use phf::phf_set;

use crate::diagnostics::diagnostic_builder::{action, diag, edit};
use crate::diagnostics::IdeDiagnostic;

static KEYWORDS: phf::Set<&'static str> = phf_set! {
    "PROGRAM", "END_PROGRAM",
    "CONFIGURATION", "END_CONFIGURATION",
    "RESOURCE", "END_RESOURCE",
    "NAMESPACE", "END_NAMESPACE",
    "USING",
    "CLASS", "END_CLASS",
    "INTERFACE", "END_INTERFACE",
    "FUNCTION", "END_FUNCTION",
    "FUNCTION_BLOCK", "END_FUNCTION_BLOCK",
    "TYPE", "END_TYPE",
    "VAR", "END_VAR",
    "VAR_INPUT",
};

pub fn add_fixes_to_parse_errors(
    db: &dyn auto_lsp::default::db::BaseDatabase,
    file: &auto_lsp::default::db::File,
    errors: &mut Vec<&ParseErrorAccumulator>,
) -> Vec<IdeDiagnostic> {
    errors
        .into_iter()
        .map(|error| match (*error).into() {
            ParseError::LexerError {
                range,
                error:
                    LexerError::Missing {
                        error: missing_error,
                        grammar_name,
                        ..
                    },
            } => {
                let mut diagnostic = diag()
                    .range(range.into())
                    .message(missing_error.to_string())
                    .source("IEC".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: auto_lsp::lsp_types::Location {
                            uri: file.url(db).clone(),
                            range: range.into(),
                        },
                        message: format!("help: add missing {grammar_name} here").into(),
                    }])
                    .call();

                diagnostic.with_fix(
                    action()
                        .title(format!("Insert missing '{}'", grammar_name))
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diagnostic.diagnostic.clone()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit()
                                .new_text(grammar_name.to_string())
                                .range(range.into())
                                .call()],
                        )])))
                        .call(),
                );
                diagnostic
            }
            ParseError::LexerError {
                range,
                error:
                    LexerError::Syntax {
                        error: syntax_error,
                        affected,
                        ..
                    },
            } => {
                if affected.len() == 1 {
                    let mut diagnostic = diag()
                        .range(range.into())
                        .message(syntax_error.to_string())
                        .source("IEC".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .related_information(vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range: range.into(),
                            },
                            message: format!("help: remove '{affected}'").into(),
                        }])
                        .call();

                    diagnostic.with_fix(
                        action()
                            .title(format!("Remove {affected}"))
                            .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                            .diagnostics(vec![diagnostic.diagnostic.clone()])
                            .is_preferred(true)
                            .edit(WorkspaceEdit::new(HashMap::from([(
                                file.url(db).clone(),
                                vec![edit().new_text("".to_string()).range(range.into()).call()],
                            )])))
                            .call(),
                    );
                    diagnostic
                } else if KEYWORDS.contains(affected.split_whitespace().next().unwrap_or("")) {
                    diag()
                        .range(range.into())
                        .message(format!("{} is a reserved keyword that is not valid in this context", affected.split_whitespace().next().unwrap_or("")))
                        .source("IEC".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .related_information(vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range: range.into(),
                            },
                            message: format!("help: remove '{}'", affected.split_whitespace().next().unwrap_or("")).into(),
                        }])
                        .call()
                } else {
                    (*error).into()
                }
            }
            _ => (*error).into(),
        })
        .collect()
}
