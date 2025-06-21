
use auto_lsp::{
    lsp_types::{
        self, CodeAction, CodeActionKind, DiagnosticRelatedInformation, DiagnosticSeverity,
        DiagnosticTag, NumberOrString, TextEdit,
    },
    tree_sitter,
};

use crate::diagnostics::IdeDiagnostic;

pub enum RangeKind {
    TreeSitter(tree_sitter::Range),
    Lsp(lsp_types::Range),
}

impl RangeKind {
    pub fn as_lsp(&self) -> lsp_types::Range {
        match self {
            RangeKind::TreeSitter(range) => lsp_types::Range {
                start: lsp_types::Position::new(
                    range.start_point.row as u32,
                    range.start_point.column as u32,
                ),
                end: lsp_types::Position::new(
                    range.end_point.row as u32,
                    range.end_point.column as u32,
                ),
            },
            RangeKind::Lsp(range) => *range,
        }
    }
}

impl From<RangeKind> for lsp_types::Range {
    fn from(range: RangeKind) -> Self {
        range.as_lsp()
    }
}

impl From<tree_sitter::Range> for RangeKind {
    fn from(range: tree_sitter::Range) -> Self {
        RangeKind::TreeSitter(range)
    }
}

impl From<lsp_types::Range> for RangeKind {
    fn from(range: lsp_types::Range) -> Self {
        RangeKind::Lsp(range)
    }
}

#[bon::builder]
pub fn diag(
    range: RangeKind,
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
        diagnostic: auto_lsp::lsp_types::Diagnostic {
            range: range.as_lsp(),
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
pub fn edit(range: RangeKind, new_text: String) -> TextEdit {
    TextEdit::new(
        range.as_lsp(),
        new_text,
    )
}