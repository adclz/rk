use ariadne::{Label, Report, Source};
use auto_lsp::{
    default::db::{BaseDatabase, file::File},
    lsp_types::{DiagnosticSeverity, NumberOrString, Url},
};
use yansi::Paint;

use crate::IdeDiagnostic;

impl IdeDiagnostic {
    pub fn create_report<'report>(
        &self,
        db: &'report dyn BaseDatabase,
        url: &'report Url,
        content: &'report str,
        config: Option<ariadne::Config>,
        format: bool,
    ) -> Report<'report, (&'report str, std::ops::Range<usize>)> {
        let error_kind = match &self.diagnostic.severity {
            Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR) => ariadne::ReportKind::Error,
            Some(auto_lsp::lsp_types::DiagnosticSeverity::WARNING) => ariadne::ReportKind::Warning,
            _ => ariadne::ReportKind::Error,
        };

        let source = Source::from(content);
        let range = self.diagnostic.range;
        let start_line = source.line(range.start.line as usize).unwrap().offset();
        let end_line = source.line(range.end.line as usize).unwrap().offset();
        let start = start_line + range.start.character as usize;
        let end = end_line + range.end.character as usize;

        let mut report = Report::build(error_kind, (url.as_str(), start..end));

        if let Some(config) = config {
            report = report.with_config(config);
        }

        match (&self.diagnostic.code, &self.code_desc) {
            (Some(code), Some(desc)) => {
                let code_str = match code {
                    NumberOrString::Number(n) => n.to_string(),
                    NumberOrString::String(s) => s.to_string(),
                };
                report = report.with_code(code_str).with_message(desc.to_string());
            }
            (Some(code), None) => {
                let code_str = match code {
                    NumberOrString::Number(n) => n.to_string(),
                    NumberOrString::String(s) => s.to_string(),
                };
                report = report.with_code(code_str);
            }
            _ => {
                report = report.with_message(self.diagnostic.message.clone());
            }
        }

        report.add_label(
            Label::new((url.as_str(), start..end))
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
