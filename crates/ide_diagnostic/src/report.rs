use ariadne::{Label, Report, Source};
use auto_lsp::{
    default::db::BaseDatabase,
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
        if !format {
            yansi::disable();
        }
        let error_kind = match &self.diagnostic.severity {
            Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR) => ariadne::ReportKind::Error,
            Some(auto_lsp::lsp_types::DiagnosticSeverity::WARNING) => ariadne::ReportKind::Warning,
            Some(auto_lsp::lsp_types::DiagnosticSeverity::INFORMATION) => {
                ariadne::ReportKind::Custom("Info", yansi::Color::Blue)
            }
            Some(auto_lsp::lsp_types::DiagnosticSeverity::HINT) => {
                ariadne::ReportKind::Custom("Hint", yansi::Color::BrightBlue)
            }
            _ => ariadne::ReportKind::Error,
        };

        let span = char_range(content, &self.diagnostic.range);

        let mut report = Report::build(error_kind, (url.as_str(), span.clone()));

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
            Label::new((url.as_str(), span))
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
            let document = related.file.document(db);
            let span = char_range(
                &document.texter.text,
                &document
                    .denormalize_range(&related.range)
                    .unwrap_or_default(),
            );
            report.add_label(
                Label::new((related.file.url(db).as_str(), span))
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

/// Ariadne addresses a source by character, where a tree-sitter range counts
/// bytes. One non-ASCII character earlier in the file slides every byte-built
/// label forward onto the wrong line, which is what an em-dash in a doc
/// comment did to every `declared here` in the standard library. The line and
/// the column survive the difference, so the offset is rebuilt from them.
fn char_range(content: &str, range: &auto_lsp::lsp_types::Range) -> std::ops::Range<usize> {
    let source = Source::from(content);
    let start = source
        .line(range.start.line as usize)
        .map(|l| l.offset())
        .unwrap_or(0)
        + range.start.character as usize;
    let end = source
        .line(range.end.line as usize)
        .map(|l| l.offset())
        .unwrap_or(content.len().saturating_sub(1))
        + range.end.character as usize;
    start..end
}
