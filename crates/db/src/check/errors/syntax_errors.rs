use std::collections::HashMap;

use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::{DiagnosticRelatedInformation, WorkspaceEdit},
};

use crate::check::{
    diagnostic_builder::{action, diag, edit},
    IdeDiagnostic,
};

/// Missing node in the parse tree
///
/// This error is emitted by tree-sitter when a node is expected but not found.
pub fn missing_node(
    db: &dyn BaseDatabase,
    file: File,
    span: Span,
    missing_error: &str,
    grammar_name: &str,
) -> IdeDiagnostic {
    let mut diagnostic = diag()
        .range(span.clone())
        .message(missing_error.to_string())
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: file.url(db).clone(),
                range: span.clone().into(),
            },
            message: format!("help: add missing {grammar_name} here"),
        }])
        .call();

    // If the grammar name is an identifier, suggest inserting it.
    if grammar_name.contains("identifier") {
        diagnostic.with_fix(
            action()
                .title(format!("Insert missing '{grammar_name}'"))
                .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                .diagnostics(vec![diagnostic.diagnostic.clone()])
                .is_preferred(true)
                .edit(WorkspaceEdit::new(HashMap::from([(
                    file.url(db).clone(),
                    vec![edit()
                        .new_text(format!(" {grammar_name}"))
                        .range(span)
                        .call()],
                )])))
                .call(),
        );
    };
    diagnostic
}

/// Unexpected character in the parse tree
/// This error is emitted by tree-sitter when a character is found that is not expected in the current
pub fn unexpected_char(
    db: &dyn BaseDatabase,
    file: File,
    span: Span,
    affected: &str,
    syntax_error: &str,
) -> IdeDiagnostic {
    let mut diagnostic = diag()
        .range(span.clone())
        .message(syntax_error.to_string())
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: file.url(db).clone(),
                range: span.clone().into(),
            },
            message: format!("help: remove '{affected}'"),
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
                vec![edit().new_text("".to_string()).range(span.clone()).call()],
            )])))
            .call(),
    );
    diagnostic
}

/// Usage of a reserved keyword in an invalid context
pub fn unexpected_keyword(
    db: &dyn BaseDatabase,
    file: File,
    span: Span,
    affected: &str,
) -> IdeDiagnostic {
    diag()
        .range(span.clone())
        .message(format!(
            "{} is a reserved keyword that is not valid in this context",
            affected.split_whitespace().next().unwrap_or("")
        ))
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: file.url(db).clone(),
                range: span.clone().into(),
            },
            message: format!(
                "help: remove or replace '{}'",
                affected.split_whitespace().next().unwrap_or("")
            ),
        }])
        .call()
}
