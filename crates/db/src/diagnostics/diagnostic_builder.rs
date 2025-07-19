use auto_lsp::{
    core::span::Span, default::db::file::File, lsp_types::{
        self, CodeAction, CodeActionKind, DiagnosticRelatedInformation, DiagnosticSeverity,
        DiagnosticTag, NumberOrString, TextEdit,
    }
};

use crate::diagnostics::IdeDiagnostic;

#[bon::builder]
pub fn diag<'a>(
    file: File,
    range: Span,
    message: String,
    source: Option<String>,
    severity: Option<DiagnosticSeverity>,
    tags: Option<Vec<DiagnosticTag>>,
    code_description: Option<lsp_types::CodeDescription>,
    code: Option<NumberOrString>,
    related_information: Option<Vec<DiagnosticRelatedInformation>>,
    fixes: Option<Vec<CodeAction>>,
) -> IdeDiagnostic {
    IdeDiagnostic {
        file,
        diagnostic: auto_lsp::lsp_types::Diagnostic {
            range: range.into(),
            severity,
            source,
            message,
            code,
            code_description,
            related_information,
            tags,
            data: None,
        },
        fixes: fixes.unwrap_or_default(),
    }
}

#[bon::builder]
pub fn action(
    title: String,
    kind: Option<CodeActionKind>,
    diagnostics: Option<Vec<auto_lsp::lsp_types::Diagnostic>>,
    is_preferred: Option<bool>,
    edit: Option<auto_lsp::lsp_types::WorkspaceEdit>,
    command: Option<auto_lsp::lsp_types::Command>,
    disabled: Option<auto_lsp::lsp_types::CodeActionDisabled>,
) -> CodeAction {
    CodeAction {
        title,
        kind,
        diagnostics,
        is_preferred,
        edit,
        command,
        data: None,
        disabled,
    }
}

#[bon::builder]
pub fn edit<'a>(range: Span, new_text: String) -> TextEdit {
    TextEdit::new(range.into(), new_text)
}
