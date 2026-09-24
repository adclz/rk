//! Diagnostic collection and rendering for the CLI: [`collect_diagnostics`]
//! runs the per-file checks in parallel; a [`DiagnosticReporter`] renders
//! them and tallies the counts.

use std::io::Write;
use std::path::{Path, PathBuf};

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, Url};
use db::RootDatabase;
use ide_diagnostic::IdeDiagnostic;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::reports::sources;

/// Collect diagnostics for every workspace file, in parallel; `with_linter`
/// runs the configured passes too (`rk check`, not codegen).
pub fn collect_diagnostics(
    db: &RootDatabase,
    with_linter: bool,
) -> Vec<(File, Vec<IdeDiagnostic>)> {
    // A workspace that never mentions the linter still gets the recommended
    // rules; `[linter]` tunes the set.
    let linter_config = with_linter.then(|| {
        db::config_file::get_config(db)
            .linter
            .clone()
            .unwrap_or_default()
    });

    // Ordered, so the diagnostics come out in the same sequence every run —
    // rayon's collect preserves the input order, it just cannot invent one.
    crate::file_order::ordered_files(db)
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let mut diagnostics = hir::check::diagnostics_for_file(db, file).as_ref().clone();
            if let Some(ref linter_config) = linter_config {
                linter::lint_file(db, file, linter_config, &mut diagnostics);
            }
            (file, diagnostics)
        })
        .collect()
}

/// The ERRORS of the library files, and only the errors: the editor hides
/// library diagnostics on purpose, but the build cannot, since a broken
/// library lowers into an invalid module with no message naming the
/// cause.
pub fn collect_library_errors(db: &RootDatabase) -> Vec<(File, Vec<IdeDiagnostic>)> {
    crate::file_order::ordered_library_files(db)
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let errors: Vec<IdeDiagnostic> = hir::check::diagnostics_for_file(db, file)
                .iter()
                .filter(|d| {
                    matches!(
                        d.diagnostic.severity,
                        Some(DiagnosticSeverity::ERROR) | None
                    )
                })
                .cloned()
                .collect();
            (file, errors)
        })
        .filter(|(_, errors)| !errors.is_empty())
        .collect()
}

/// Renders collected diagnostics and tallies the counts, with
/// workspace-relative paths. The format decides the wire shape: ariadne
/// reports, one concise line per diagnostic, or one JSON object per line.
pub struct DiagnosticReporter<'db> {
    db: &'db RootDatabase,
    workspace_path: PathBuf,
    config: ariadne::Config,
    format: OutputFormat,
}

use crate::cli::OutputFormat;

/// Per-severity tallies for one run; a missing severity counts as an error
/// (the LSP convention), so commands can base their exit code on `errors`
/// alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticCounts {
    pub errors: i32,
    pub warnings: i32,
    pub infos: i32,
    pub hints: i32,
}

/// A relative path rendered with `/` on every platform, since it is
/// printed, compared and snapshotted.
pub(crate) fn slash_path(p: &std::path::Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

impl DiagnosticCounts {
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    fn add(&mut self, severity: Option<DiagnosticSeverity>) {
        match severity {
            Some(DiagnosticSeverity::WARNING) => self.warnings += 1,
            Some(DiagnosticSeverity::INFORMATION) => self.infos += 1,
            Some(DiagnosticSeverity::HINT) => self.hints += 1,
            // ERROR, or unset — an unset severity is an error by convention.
            _ => self.errors += 1,
        }
    }

    fn merge(&mut self, other: DiagnosticCounts) {
        self.errors += other.errors;
        self.warnings += other.warnings;
        self.infos += other.infos;
        self.hints += other.hints;
    }
}

impl<'db> DiagnosticReporter<'db> {
    pub fn new(db: &'db RootDatabase, workspace: &Path) -> Self {
        let canonical =
            std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
        // On Windows, canonicalize returns a verbatim path (`\\?\C:\...`);
        // diagnostics' paths arrive through a Url round-trip that drops it, so
        // this path takes the same round-trip.
        let workspace_path = Url::from_file_path(&canonical)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .unwrap_or(canonical);
        // Color follows the one process-wide decision made at startup
        // (`ui::init_output`).
        let config = ariadne::Config::new()
            .with_color(yansi::is_enabled())
            .with_tab_width(2);
        Self {
            db,
            workspace_path,
            config,
            format: OutputFormat::Full,
        }
    }

    /// Select the output format (default: [`OutputFormat::Full`]).
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Render every file's diagnostics to `out`, returning the tallies; the
    /// source cache spans all files so cross-file related info renders.
    pub fn report_files(
        &self,
        per_file: &[(File, Vec<IdeDiagnostic>)],
        out: &mut dyn Write,
    ) -> DiagnosticCounts {
        // Library sources ride along: a workspace diagnostic may point its
        // related span into a library.
        let caches: Vec<(&str, &str)> = crate::file_order::ordered_files_with_libraries(self.db)
            .into_iter()
            .map(|file| (file.url(self.db).as_str(), file.document(self.db).as_str()))
            .collect();

        let mut counts = DiagnosticCounts::default();
        for (file, diagnostics) in per_file {
            if diagnostics.is_empty() {
                continue;
            }
            counts.merge(self.render(
                file.url(self.db),
                &file.document(self.db).texter.text,
                diagnostics,
                &caches,
                out,
            ));
        }
        counts
    }

    /// Render a single external source's diagnostics (e.g. config-file errors,
    /// which aren't a workspace [`File`]), returning the per-severity tallies.
    pub fn report_external(
        &self,
        url: &Url,
        content: &str,
        diagnostics: &[IdeDiagnostic],
        out: &mut dyn Write,
    ) -> DiagnosticCounts {
        let caches = [(url.as_str(), content)];
        self.render(url, content, diagnostics, &caches, out)
    }

    /// Render one source's diagnostics against `caches`, tallying counts.
    fn render(
        &self,
        url: &Url,
        content: &str,
        diagnostics: &[IdeDiagnostic],
        caches: &[(&str, &str)],
        out: &mut dyn Write,
    ) -> DiagnosticCounts {
        let url_str = url.as_str();
        let rel_path = self.rel_path(url);

        let mut counts = DiagnosticCounts::default();
        for diagnostic in diagnostics {
            counts.add(diagnostic.diagnostic.severity);

            match self.format {
                OutputFormat::Full => {
                    let report =
                        diagnostic.create_report(self.db, url, content, Some(self.config), true);
                    let mut buffer = vec![];
                    report
                        .write(sources(caches.iter().copied()), &mut buffer)
                        .expect("failed to write report");

                    let output = String::from_utf8_lossy(&buffer);
                    // Render to the caller's sink: stderr for CLI commands, or an
                    // in-memory buffer for a tool whose stdout is a transport.
                    let _ = write!(out, "{}", output.replace(url_str, &rel_path));
                }
                OutputFormat::Concise => self.render_concise(&rel_path, diagnostic, out),
                OutputFormat::JsonLines => self.render_json_line(&rel_path, diagnostic, out),
            }
        }
        counts
    }

    /// Workspace-relative path for `url` (the URL itself for other sources),
    /// always `/`-separated: this string is an identifier on the wire, keyed
    /// on by editors, CI annotations and snapshots.
    fn rel_path(&self, url: &Url) -> String {
        url.to_file_path()
            .ok()
            .and_then(|abs_path| {
                abs_path
                    .strip_prefix(&self.workspace_path)
                    .ok()
                    .map(slash_path)
            })
            .unwrap_or_else(|| url.as_str().to_string())
    }

    /// One machine-stable line: `FILE:LINE:COL: severity[CODE]: message`
    /// (1-based), the first quick-fix title appended as `: help: <title>`.
    /// The message is last and colon-tolerant.
    fn render_concise(&self, rel_path: &str, diagnostic: &IdeDiagnostic, out: &mut dyn Write) {
        let d = &diagnostic.diagnostic;
        let line = d.range.start.line + 1;
        let col = d.range.start.character + 1;
        let severity = severity_str(d.severity);
        let code = code_str(d.code.as_ref());
        let message = d.message.replace('\n', " ");

        let _ = write!(out, "{rel_path}:{line}:{col}: {severity}");
        if let Some(code) = code {
            let _ = write!(out, "[{code}]");
        }
        let _ = write!(out, ": {message}");
        if let Some(fix) = diagnostic.fixes().first() {
            let _ = write!(out, ": help: {}", fix.title.replace('\n', " "));
        }
        let _ = writeln!(out);
    }

    /// One JSON object per line (NDJSON): positions 1-based, severities the same
    /// strings as concise, `help` = quick-fix titles, `related` resolved to
    /// workspace-relative file:line:col.
    fn render_json_line(&self, rel_path: &str, diagnostic: &IdeDiagnostic, out: &mut dyn Write) {
        let d = &diagnostic.diagnostic;
        let related: Vec<JsonRelated> = diagnostic
            .related()
            .iter()
            .map(|r| {
                let range = r
                    .file
                    .document(self.db)
                    .denormalize_range(&r.range)
                    .unwrap_or_default();
                JsonRelated {
                    file: self.rel_path(r.file.url(self.db)),
                    line: range.start.line + 1,
                    col: range.start.character + 1,
                    message: r.message.clone(),
                }
            })
            .collect();

        let record = JsonDiagnostic {
            file: rel_path,
            line: d.range.start.line + 1,
            col: d.range.start.character + 1,
            end_line: d.range.end.line + 1,
            end_col: d.range.end.character + 1,
            severity: severity_str(d.severity),
            code: code_str(d.code.as_ref()),
            message: &d.message,
            source: d.source.as_deref(),
            notes: diagnostic.notes().iter().map(String::as_str).collect(),
            help: diagnostic
                .fixes()
                .iter()
                .map(|f| f.title.as_str())
                .collect(),
            related,
        };
        if let Ok(json) = serde_json::to_string(&record) {
            let _ = writeln!(out, "{json}");
        }
    }
}

/// Severity as the stable lowercase token used by both machine formats.
/// LSP leaves severity optional; an unset severity is an error by convention.
fn severity_str(severity: Option<DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(DiagnosticSeverity::WARNING) => "warning",
        Some(DiagnosticSeverity::INFORMATION) => "info",
        Some(DiagnosticSeverity::HINT) => "hint",
        _ => "error",
    }
}

/// The diagnostic code (`E0301`, `L0303`) as text, if any.
fn code_str(code: Option<&auto_lsp::lsp_types::NumberOrString>) -> Option<String> {
    match code {
        Some(auto_lsp::lsp_types::NumberOrString::String(s)) => Some(s.clone()),
        Some(auto_lsp::lsp_types::NumberOrString::Number(n)) => Some(n.to_string()),
        None => None,
    }
}

/// NDJSON record for [`OutputFormat::JsonLines`]. Field order is part of the
/// wire shape; additions go at the END so line-oriented consumers keep working.
#[derive(serde::Serialize)]
struct JsonDiagnostic<'a> {
    file: &'a str,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
    severity: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    help: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related: Vec<JsonRelated>,
}

#[derive(serde::Serialize)]
struct JsonRelated {
    file: String,
    line: u32,
    col: u32,
    message: String,
}
