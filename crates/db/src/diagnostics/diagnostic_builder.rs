use std::{borrow::Cow, ops::Deref};

use auto_lsp::{
    lsp_types::{
        self, CodeAction, CodeActionKind, DiagnosticRelatedInformation, DiagnosticSeverity,
        DiagnosticTag, NumberOrString, TextEdit,
    },
    tree_sitter,
};

use crate::diagnostics::IdeDiagnostic;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RangeKind<'a>(Cow<'a, tree_sitter::Range>);

impl RangeKind<'_> {
    pub fn as_lsp(&self) -> lsp_types::Range {
        lsp_types::Range {
            start: lsp_types::Position::new(
                self.0.start_point.row as u32,
                self.0.start_point.column as u32,
            ),
            end: lsp_types::Position::new(
                self.0.end_point.row as u32,
                self.0.end_point.column as u32,
            ),
        }
    }
}

impl Deref for RangeKind<'_> {
    type Target = tree_sitter::Range;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<RangeKind<'_>> for lsp_types::Range {
    fn from(range: RangeKind) -> Self {
        range.as_lsp()
    }
}

impl From<tree_sitter::Range> for RangeKind<'_> {
    fn from(range: tree_sitter::Range) -> Self {
        Self(Cow::Owned(range))
    }
}

impl<'a> From<&'a tree_sitter::Range> for RangeKind<'a> {
    fn from(range: &'a tree_sitter::Range) -> Self {
        Self(Cow::Borrowed(range))
    }
}

impl PartialOrd for RangeKind<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RangeKind<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_lsp().start.cmp(&other.as_lsp().start)
    }
}

impl PartialEq<tree_sitter::Range> for RangeKind<'_> {
    fn eq(&self, other: &tree_sitter::Range) -> bool {
        self.as_lsp().start.character == other.start_point.column as u32
            && self.as_lsp().start.line == other.start_point.row as u32
            && self.as_lsp().end.character == other.end_point.column as u32
            && self.as_lsp().end.line == other.end_point.row as u32
    }
}

impl PartialOrd<tree_sitter::Range> for RangeKind<'_> {
    fn partial_cmp(&self, other: &tree_sitter::Range) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other.into()))
    }
}

impl PartialEq<lsp_types::Range> for RangeKind<'_> {
    fn eq(&self, other: &lsp_types::Range) -> bool {
        self.as_lsp().start.character == other.start.character
            && self.as_lsp().start.line == other.start.line
            && self.as_lsp().end.character == other.end.character
            && self.as_lsp().end.line == other.end.line
    }
}

pub(crate) enum OneOf<T, U> {
    T(T),
    U(U),
}

#[bon::builder]
pub fn diag<'a>(
    range: OneOf<RangeKind<'a>, lsp_types::Range>,
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
            range: match range {
                OneOf::T(range) => range.as_lsp(),
                OneOf::U(range) => range,
            },
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
pub fn edit<'a>(range: OneOf<RangeKind<'a>, lsp_types::Range>, new_text: String) -> TextEdit {
    TextEdit::new(match range {
        OneOf::T(range) => range.as_lsp(),
        OneOf::U(range) => range,
    }, new_text)
}
