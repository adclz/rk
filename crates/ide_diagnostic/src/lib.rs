use ariadne::{Label, Report, Source};
use auto_lsp::{
    core::{errors::ParseErrorAccumulator, span::Span},
    default::db::{BaseDatabase, file::File},
    lsp_types::{
        self, CodeAction, CodeActionKind, Diagnostic, DiagnosticRelatedInformation,
        DiagnosticSeverity, DiagnosticTag, Location, NumberOrString, Range, TextEdit,
    },
};
use yansi::Paint;

#[derive(Clone, Debug)]
pub struct IdeDiagnostic {
    pub diagnostic: auto_lsp::lsp_types::Diagnostic,
    related: Vec<Related>,
    fixes: Vec<auto_lsp::lsp_types::CodeAction>,
    notes: Vec<String>,
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

impl IdeDiagnostic {
    pub fn create_report<'report>(
        &self,
        db: &'report dyn BaseDatabase,
        file: File,
        config: Option<ariadne::Config>,
        format: bool,
    ) -> Report<'report, (&'report str, std::ops::Range<usize>)> {
        let error_kind = match &self.diagnostic.severity {
            Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR) => ariadne::ReportKind::Error,
            Some(auto_lsp::lsp_types::DiagnosticSeverity::WARNING) => ariadne::ReportKind::Warning,
            _ => ariadne::ReportKind::Error,
        };

        let source = Source::from(file.document(db).as_str());
        let range = self.diagnostic.range;
        let start_line = source.line(range.start.line as usize).unwrap().offset();
        let end_line = source.line(range.end.line as usize).unwrap().offset();
        let start = start_line + range.start.character as usize;
        let end = end_line + range.end.character as usize;

        let mut report = Report::build(error_kind, (file.url(db).as_str(), start..end));

        if let Some(config) = config {
            report = report.with_config(config);
        }

        report.add_label(
            Label::new((file.url(db).as_str(), start..end))
                .with_message(match format {
                    true => Paint::bold(&self.diagnostic.message).to_string(),
                    false => self.diagnostic.message.clone(),
                })
                .with_color(match self.diagnostic.severity {
                    Some(DiagnosticSeverity::ERROR) => ariadne::Color::Red,
                    Some(DiagnosticSeverity::WARNING) => ariadne::Color::Yellow,
                    Some(DiagnosticSeverity::INFORMATION) => ariadne::Color::Blue,
                    Some(DiagnosticSeverity::HINT) => ariadne::Color::Cyan,
                    _ => ariadne::Color::Red,
                }),
        );

        for related in &self.related {
            report.add_label(
                Label::new((
                    related.file.url(db).as_str(),
                    related.range.start_byte..related.range.end_byte,
                ))
                .with_message(match format {
                    true => Paint::italic(&related.message).to_string(),
                    false => related.message.clone(),
                })
                .with_color(ariadne::Color::BrightBlue),
            )
        }

        for fix in &self.fixes {
            report.add_help(fix.title.to_string());
        }

        for note in self.notes.iter() {
            report.add_note(note.to_string());
        }

        if let Some(code) = &self.diagnostic.code {
            report.with_code(match code {
                NumberOrString::Number(n) => n.to_string(),
                NumberOrString::String(s) => s.to_string(),
            })
        } else {
            report
        }
        .finish()
    }
}

#[bon::builder]
pub fn diag(
    range: Span,
    message: String,
    source: Option<String>,
    severity: Option<DiagnosticSeverity>,
    tags: Option<Vec<DiagnosticTag>>,
    code_description: Option<lsp_types::CodeDescription>,
    code: Option<NumberOrString>,
) -> IdeDiagnostic {
    IdeDiagnostic {
        diagnostic: auto_lsp::lsp_types::Diagnostic {
            range: range.into(),
            severity,
            source,
            message,
            code,
            code_description,
            related_information: None,
            tags,
            data: None,
        },
        fixes: vec![],
        related: vec![],
        notes: vec![],
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
