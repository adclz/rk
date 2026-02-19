use auto_lsp::{
    core::{errors::ParseErrorAccumulator, span::Span},
    default::db::{BaseDatabase, file::File},
    lsp_types::{
        self, CodeAction, CodeActionKind, CodeDescription, Diagnostic,
        DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location, NumberOrString,
        Range, TextEdit, Url,
    },
};

pub mod report;

#[derive(Clone, Debug)]
pub struct IdeDiagnostic {
    pub diagnostic: auto_lsp::lsp_types::Diagnostic,
    related: Vec<Related>,
    fixes: Vec<auto_lsp::lsp_types::CodeAction>,
    notes: Vec<String>,
    code_desc: Option<&'static str>,
}
impl IdeDiagnostic {
    pub fn inner(&self) -> Diagnostic {
        self.diagnostic.clone()
    }

    pub fn fixes(&self) -> &[CodeAction] {
        &self.fixes
    }

    pub fn range(&self) -> &Range {
        &self.diagnostic.range
    }

    pub fn to_lsp_diagnostic(&self, db: &dyn BaseDatabase) -> Diagnostic {
        let related = self
            .related
            .iter()
            .map(|r| DiagnosticRelatedInformation {
                message: r.message.clone(),
                location: Location::new(r.file.url(db).to_owned(), r.range.lsp()),
            })
            .collect();

        let message = match self.notes.len() {
            0 => self.diagnostic.message.clone(),
            _ => {
                let mut message = self.diagnostic.message.clone();
                message.push_str("\n\nNote: ");
                message.push_str(&self.notes.join("\n"));
                message
            }
        };

        Diagnostic {
            range: self.diagnostic.range,
            severity: self.diagnostic.severity,
            code: self.diagnostic.code.clone(),
            code_description: self.diagnostic.code_description.clone(),
            source: self.diagnostic.source.clone(),
            message,
            related_information: Some(related),
            tags: self.diagnostic.tags.clone(),
            data: self.diagnostic.data.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Related {
    pub message: String,
    pub file: File,
    pub range: Span,
}

impl Related {
    pub fn new(message: String, file: File, range: Span) -> Self {
        Self {
            message,
            file,
            range,
        }
    }
}

impl PartialEq for IdeDiagnostic {
    fn eq(&self, other: &Self) -> bool {
        self.diagnostic == other.diagnostic
    }
}

impl Eq for IdeDiagnostic {}

impl std::hash::Hash for IdeDiagnostic {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.diagnostic.message.hash(state);
    }
}

impl IdeDiagnostic {
    pub fn new(diagnostic: auto_lsp::lsp_types::Diagnostic) -> Self {
        Self {
            diagnostic,
            related: vec![],
            fixes: vec![],
            notes: vec![],
            code_desc: None,
        }
    }

    pub fn with_related(&mut self, related: Related) {
        self.related.push(related);
    }

    pub fn with_note(&mut self, message: String) {
        self.notes.push(message);
    }

    pub fn with_fix(&mut self, fix: auto_lsp::lsp_types::CodeAction) {
        self.fixes.push(fix);
    }
}

impl From<IdeDiagnostic> for auto_lsp::lsp_types::Diagnostic {
    fn from(d: IdeDiagnostic) -> Self {
        d.diagnostic
    }
}

impl From<&IdeDiagnostic> for auto_lsp::lsp_types::Diagnostic {
    fn from(d: &IdeDiagnostic) -> Self {
        d.diagnostic.clone()
    }
}

impl From<auto_lsp::lsp_types::Diagnostic> for IdeDiagnostic {
    fn from(d: auto_lsp::lsp_types::Diagnostic) -> Self {
        IdeDiagnostic::new(d)
    }
}

impl From<&ParseErrorAccumulator> for IdeDiagnostic {
    fn from(e: &ParseErrorAccumulator) -> Self {
        IdeDiagnostic::new(e.0.clone().into())
    }
}

pub trait ErrorCode {
    fn code(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn url(&self) -> CodeDescription {
        CodeDescription {
            href: Url::parse(&format!(
                "https://iec-3.github.io/rk/errors/{}.html",
                self.code()
            ))
            .unwrap(),
        }
    }
}

struct Desc {
    pub code: Option<&'static str>,
    pub description: Option<&'static str>,
    pub url: Option<lsp_types::CodeDescription>,
}

#[bon::builder]
pub fn diag(
    range: Span,
    message: String,
    source: Option<String>,
    severity: Option<DiagnosticSeverity>,
    tags: Option<Vec<DiagnosticTag>>,

    #[builder(with = |desc: &impl ErrorCode| Desc {
        code: Some(desc.code()),
        description: Some(desc.description()),
        url: Some(desc.url()),
     })]
    desc: Desc,
) -> IdeDiagnostic {
    IdeDiagnostic {
        diagnostic: auto_lsp::lsp_types::Diagnostic {
            range: range.into(),
            severity,
            source,
            message,
            code: desc.code.map(|c| NumberOrString::String(c.to_owned())),
            code_description: desc.url,
            tags,
            related_information: None,
            data: None,
        },
        fixes: vec![],
        related: vec![],
        notes: vec![],
        code_desc: desc.description,
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
pub fn edit(range: Span, new_text: String) -> TextEdit {
    TextEdit::new(range.into(), new_text)
}
