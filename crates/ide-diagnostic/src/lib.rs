use ariadne::{ColorGenerator, Fmt, Label, Report, Source};
use auto_lsp::{
    core::{errors::ParseErrorAccumulator, span::Span},
    default::db::{file::File, BaseDatabase},
    lsp_types::{
        self, CodeAction, CodeActionKind, DiagnosticRelatedInformation, DiagnosticSeverity,
        DiagnosticTag, NumberOrString, TextEdit,
    },
};

#[derive(Clone)]
pub struct IdeDiagnostic {
    pub diagnostic: auto_lsp::lsp_types::Diagnostic,
    pub fixes: Vec<auto_lsp::lsp_types::CodeAction>,
}

impl IdeDiagnostic {
    pub fn new(diagnostic: auto_lsp::lsp_types::Diagnostic) -> Self {
        Self {
            diagnostic,
            fixes: vec![],
        }
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
    pub fn create_report<'db>(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> Report<'db, (&'db str, std::ops::Range<usize>)> {
        let mut colors = ColorGenerator::new();
        let curr_color = colors.next();

        let error_kind = match &self.diagnostic.severity {
            Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR) => ariadne::ReportKind::Error,
            Some(auto_lsp::lsp_types::DiagnosticSeverity::WARNING) => ariadne::ReportKind::Warning,
            _ => ariadne::ReportKind::Advice,
        };

        let source = Source::from(file.document(db).as_str());
        let range = self.diagnostic.range;
        let start_line = source.line(range.start.line as usize).unwrap().offset();
        let end_line = source.line(range.end.line as usize).unwrap().offset();
        let start = start_line + range.start.character as usize;
        let end = end_line + range.end.character as usize;

        let mut report = Report::build(error_kind, (file.url(db).as_str(), start..end));
        report.add_label(
            Label::new((file.url(db).as_str(), start..end))
                .with_message(format!(
                    "{}",
                    self.diagnostic.message.to_owned().fg(curr_color)
                ))
                .with_color(curr_color),
        );

        if let Some(related) = &self.diagnostic.related_information {
            for related in related {
                report.add_help(format!("{}", related.message.to_owned().fg(curr_color)))
            }
        }

        if self.fixes.len() > 0 {
            report.add_note(format!("{} fix(es) available", self.fixes.len()));
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
pub fn diag<'a>(
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
