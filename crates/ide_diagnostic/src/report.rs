// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::fmt;

use ariadne::{Label, Report, Source};
use auto_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Url};
use yansi::Paint;

use crate::IdeDiagnostic;

/// The files the reports of one run point into, each split into lines the
/// first time one does and kept for the others. A file was split again for
/// each report and each of its labels, so a file of many reports cost the
/// square of its length. The reports are written against it: it is
/// ariadne's cache of their sources.
#[derive(Default)]
pub struct Sources<'a> {
    by_url: HashMap<&'a str, Source<&'a str>>,
}

impl<'a> Sources<'a> {
    /// The lines of `text`, the file at `url`.
    fn of(&mut self, url: &'a str, text: &'a str) -> &Source<&'a str> {
        self.by_url.entry(url).or_insert_with(|| Source::from(text))
    }
}

impl<'a> ariadne::Cache<&'a str> for Sources<'a> {
    type Storage = &'a str;

    fn fetch(&mut self, id: &&'a str) -> Result<&Source<&'a str>, impl fmt::Debug> {
        self.by_url
            .get(id)
            .ok_or_else(|| format!("failed to fetch source '{id}'"))
    }

    fn display<'b>(&self, id: &'b &'a str) -> Option<impl fmt::Display + 'b> {
        Some(*id)
    }
}

impl IdeDiagnostic {
    /// The report of this diagnostic, on `content`, the text of the file at
    /// `url`. The files its labels point into are added to `sources`, which
    /// it is then written against.
    pub fn create_report<'report>(
        &self,
        db: &'report dyn auto_lsp::salsa::Database,
        url: &'report Url,
        content: &'report str,
        sources: &mut Sources<'report>,
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

        let span = char_range(
            sources.of(url.as_str(), content),
            content,
            &self.diagnostic.range,
        );

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
            let related_url = related.file.url(db).as_str();
            let span = char_range(
                sources.of(related_url, &document.texter.text),
                &document.texter.text,
                &document
                    .denormalize_range(&related.range)
                    .unwrap_or_default(),
            );
            report.add_label(
                Label::new((related_url, span))
                    .with_message(match format {
                        true => Paint::italic(&related.message).to_string(),
                        false => related.message.clone(),
                    })
                    .with_color(ariadne::Color::BrightBlue),
            )
        }

        for note in self.notes.iter() {
            report.add_note(note.to_string());
        }

        for help in self.helps.iter() {
            report.add_help(help.to_string());
        }

        for fix in &self.fixes {
            report.add_help(fix.title.to_string());
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
fn char_range(
    source: &Source<&str>,
    content: &str,
    range: &auto_lsp::lsp_types::Range,
) -> std::ops::Range<usize> {
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
