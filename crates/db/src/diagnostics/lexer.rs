use std::collections::HashMap;

use auto_lsp::core::errors::{LexerError, ParseError, ParseErrorAccumulator};
use auto_lsp::lsp_types::{
    self, CodeAction, DiagnosticRelatedInformation, TextEdit, WorkspaceEdit,
};

use crate::diagnostics::IdeDiagnostic;

pub fn lexer_error_with_fix(
    db: &dyn auto_lsp::default::db::BaseDatabase,
    file: &auto_lsp::default::db::File,
    errors: &mut Vec<&ParseErrorAccumulator>,
) -> Vec<IdeDiagnostic> {
    errors
        .into_iter()
        .filter_map(|error| {
            if let ParseError::LexerError {
                range,
                error: lexer_error,
            } = (*error).into()
            {
                if let LexerError::Missing {
                    range: missing_range,
                    error: missing_error,
                    grammar_name,
                } = lexer_error
                {
                    let mut diagnostic = IdeDiagnostic::new(auto_lsp::lsp_types::Diagnostic {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(
                                missing_range.start.line as u32,
                                missing_range.start.character as u32,
                            ),
                            end: lsp_types::Position::new(
                                missing_range.end.line as u32,
                                missing_range.end.character as u32,
                            ),
                        },
                        code_description: None,
                        severity: Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR),
                        code: None,
                        source: Some("IEC".into()),
                        message: missing_error.to_string(),
                        related_information: Some(vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range: lsp_types::Range {
                                    start: lsp_types::Position::new(
                                        missing_range.start.line as u32,
                                        missing_range.start.character as u32,
                                    ),
                                    end: lsp_types::Position::new(
                                        missing_range.end.line as u32,
                                        missing_range.end.character as u32,
                                    ),
                                },
                            },
                            message: format!("help: add missing {grammar_name} here").into(),
                        }]),
                        tags: None,
                        data: None,
                    });

                    diagnostic.with_fix(CodeAction {
                        title: format!("Insert missing '{}'", grammar_name),
                        kind: Some(auto_lsp::lsp_types::CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.diagnostic.clone()]),
                        is_preferred: Some(true),
                        edit: Some(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![TextEdit::new(
                                lsp_types::Range {
                                    start: lsp_types::Position::new(
                                        missing_range.start.line as u32,
                                        missing_range.start.character as u32 + 1,
                                    ),
                                    end: lsp_types::Position::new(
                                        missing_range.end.line as u32,
                                        missing_range.end.character as u32 + 1,
                                    ),
                                },
                                grammar_name.to_string(),
                            )],
                        )]))),
                        command: None,
                        data: None,
                        disabled: None,
                    });
                    return Some(diagnostic)
                } else if let LexerError::Syntax {
                    range: affected_range,
                    error: syntax_error,
                    affected,
                } = lexer_error 
                {
                    if affected.len() == 1 {
                        let mut diagnostic = IdeDiagnostic::new(auto_lsp::lsp_types::Diagnostic {
                            range: lsp_types::Range {
                                start: lsp_types::Position::new(
                                    affected_range.start.line as u32,
                                    affected_range.start.character as u32,
                                ),
                                end: lsp_types::Position::new(
                                    affected_range.end.line as u32,
                                    affected_range.end.character as u32,
                                ),
                            },
                            code_description: None,
                            severity: Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR),
                            code: None,
                            source: Some("IEC".into()),
                            message: syntax_error.to_string(),
                            related_information: Some(vec![DiagnosticRelatedInformation {
                                location: auto_lsp::lsp_types::Location {
                                    uri: file.url(db).clone(),
                                    range: lsp_types::Range {
                                start: lsp_types::Position::new(
                                    affected_range.start.line as u32,
                                    affected_range.start.character as u32,
                                ),
                                end: lsp_types::Position::new(
                                    affected_range.end.line as u32,
                                    affected_range.end.character as u32,
                                ),
                                    },
                                },
                                message: format!("help: remove '{affected}'"),
                            }]),
                            tags: None,
                            data: None,
                        });

                        diagnostic.with_fix(CodeAction {
                            title: format!("Remove {affected}"),
                            kind: Some(auto_lsp::lsp_types::CodeActionKind::QUICKFIX),
                            diagnostics: Some(vec![diagnostic.diagnostic.clone()]),
                            is_preferred: Some(true),
                            edit: Some(WorkspaceEdit::new(HashMap::from([(
                                file.url(db).clone(),
                                vec![TextEdit::new(
                                    lsp_types::Range {
                                   start: lsp_types::Position::new(
                                    affected_range.start.line as u32,
                                    affected_range.start.character as u32,
                                ),
                                end: lsp_types::Position::new(
                                    affected_range.end.line as u32,
                                    affected_range.end.character as u32,
                                ),
                                    },
                                    "".to_string(),
                                )],
                            )]))),
                            command: None,
                            data: None,
                            disabled: None,
                        });
                        return Some(diagnostic);
                    } else {
                        // Handle other LexerError variants if needed
                        return Some((*error).into());
                    }
                } else {
                    // Handle other ParseError variants if needed
                    return Some((*error).into());
                }
            }
            return Some((*error).into());
        })
        .collect()
}
