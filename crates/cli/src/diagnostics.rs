//! Diagnostic collection + rendering for the CLI.
//!
//! [`collect_diagnostics`] runs the per-file checks in parallel; a
//! [`DiagnosticReporter`] renders them with ariadne and tallies the counts.
//! Shared by `rk check` and the `build_core` codegen path (which previously
//! carried near-identical copies of this pipeline).

use std::io::Write;
use std::path::{Path, PathBuf};

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, Url};
use db::RootDatabase;
use ide_diagnostic::IdeDiagnostic;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::reports::sources;

/// Collect diagnostics for every workspace file, in parallel. With `with_linter`
/// the configured linter passes run too (used by `rk check`; codegen skips them —
/// a style lint shouldn't block a build).
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
                    // Render to the caller's sink — stderr for CLI commands, or an
                    // in-memory buffer for `build_core` (which forwards the text over the
                    // debugger transport, since stdout is the debugger transport there).
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

/// The diagnostic code (`E0301`, `L0204`) as text, if any.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;
    use crate::workspace::init_db;

    const CONFIG_TOML: &str = "[project]\nname = \"Test\"\nversion = \"0.0\"\n";
    // One E0301 (type mismatch) with a quick-fix and related info — exercises
    // every field the machine formats carry.
    const SRC: &str = "FUNCTION f : INT\nVAR x : INT; END_VAR\n    x := ULINT#5;\nEND_FUNCTION\n";

    /// Render the fixture workspace's diagnostics in `format`.
    fn render(format: OutputFormat) -> String {
        let ws = tempfile::tempdir().expect("tempdir");
        std::fs::write(ws.path().join("config.toml"), CONFIG_TOML).unwrap();
        std::fs::write(ws.path().join("main.st"), SRC).unwrap();
        // Tests must not inherit the developer's library environment.
        unsafe { std::env::set_var(db::loader::STDLIB_PATH_ENV, "") };
        let db = init_db(ws.path(), false, true).expect("init db");
        let per_file = collect_diagnostics(&db, false);
        let mut out = Vec::new();
        let counts = DiagnosticReporter::new(&db, ws.path())
            .with_format(format)
            .report_files(&per_file, &mut out);
        assert!(
            counts.has_errors(),
            "fixture must produce at least one error"
        );
        String::from_utf8(out).unwrap()
    }

    /// `FILE:LINE:COL: severity[CODE]: message` — one line, digits where digits
    /// belong, no ANSI ever (machine formats bypass the color pipeline).
    #[test]
    fn concise_is_one_regex_stable_line_per_diagnostic() {
        let out = render(OutputFormat::Concise);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1, "one diagnostic, one line: {out:?}");
        let line = lines[0];

        assert!(
            !line.contains('\x1b'),
            "no ANSI in concise output: {line:?}"
        );
        assert!(
            line.starts_with("main.st:"),
            "workspace-relative path: {line:?}"
        );
        let mut parts = line.splitn(4, ':');
        let (_file, l, c, rest) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        l.parse::<u32>().expect("line is a number");
        c.parse::<u32>().expect("col is a number");
        assert!(
            rest.starts_with(" error[E0301]: "),
            "severity[CODE]: {rest:?}"
        );
        assert!(
            line.contains(": help: "),
            "quick-fix title appended: {line:?}"
        );
    }

    /// Every line parses as JSON with the stable field set; positions 1-based.
    #[test]
    fn json_lines_parse_with_stable_fields() {
        let out = render(OutputFormat::JsonLines);
        assert!(!out.contains('\x1b'), "no ANSI in json-lines output");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1, "one diagnostic, one JSON line");

        let v: serde_json::Value = serde_json::from_str(lines[0]).expect("valid JSON");
        assert_eq!(v["file"], "main.st");
        assert_eq!(v["severity"], "error");
        assert_eq!(v["code"], "E0301");
        assert_eq!(v["line"], 3, "1-based line of `x := ULINT#5`");
        assert!(v["col"].as_u64().unwrap() >= 1, "1-based column");
        assert!(
            !v["message"].as_str().unwrap().is_empty(),
            "message present"
        );
        assert!(
            v["help"].as_array().is_some_and(|h| !h.is_empty()),
            "quick-fix titles carried: {v}"
        );
    }

    /// The trap this guards: a workspace whose only findings are lints
    /// (info/hint) must count ZERO errors — `rk check` bases its exit code on
    /// `has_errors()`, so advice alone no longer exits 1. The advice still
    /// renders (here in concise, as `info[...]`/`hint[...]` lines).
    #[test]
    fn advice_only_workspace_has_no_errors() {
        let ws = tempfile::tempdir().expect("tempdir");
        // `select = "all"` is load-bearing: `unused-variable` is info
        // severity, outside the recommended baseline.
        std::fs::write(
            ws.path().join("config.toml"),
            format!("{CONFIG_TOML}\n[linter]\nselect = \"all\"\n"),
        )
        .unwrap();
        // Valid code with an unused variable — a linter finding, not an error.
        std::fs::write(
            ws.path().join("main.st"),
            "FUNCTION f : INT\nVAR unused : INT; END_VAR\n    f := 1;\nEND_FUNCTION\n",
        )
        .unwrap();
        // Tests must not inherit the developer's library environment.
        unsafe { std::env::set_var(db::loader::STDLIB_PATH_ENV, "") };
        let db = init_db(ws.path(), false, true).expect("init db");
        let per_file = collect_diagnostics(&db, true);
        let mut out = Vec::new();
        let counts = DiagnosticReporter::new(&db, ws.path())
            .with_format(OutputFormat::Concise)
            .report_files(&per_file, &mut out);

        assert_eq!(counts.errors, 0, "no errors in an advice-only workspace");
        assert!(
            counts.infos + counts.hints > 0,
            "the unused-variable lint fired: {counts:?}"
        );
        assert!(!counts.has_errors(), "advice alone must not fail the check");
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.lines()
                .all(|l| l.contains(": info") || l.contains(": hint") || l.contains(": warning")),
            "only advice lines rendered: {out:?}"
        );
    }

    /// A bare directory of `.st` files — NO config.toml — is checkable:
    /// `init_db(require_config = false)` loads it, and analysis still runs
    /// fully (real errors reported, plus the E0217 outside-a-project hint;
    /// the HIR used to return ONLY the hint). Artifact-producing commands
    /// keep requiring a config (`require_config = true` → None).
    #[test]
    fn no_config_workspace_is_checkable_with_full_analysis() {
        let ws = tempfile::tempdir().expect("tempdir");
        std::fs::write(ws.path().join("main.st"), SRC).unwrap();

        assert!(
            init_db(ws.path(), false, true).is_none(),
            "require_config must still refuse"
        );

        // Tests must not inherit the developer's library environment.
        unsafe { std::env::set_var(db::loader::STDLIB_PATH_ENV, "") };
        let db = init_db(ws.path(), false, false).expect("configless init");
        let per_file = collect_diagnostics(&db, true);
        let mut out = Vec::new();
        let counts = DiagnosticReporter::new(&db, ws.path())
            .with_format(OutputFormat::Concise)
            .report_files(&per_file, &mut out);

        assert_eq!(
            counts.errors, 1,
            "the real E0301 is still found: {counts:?}"
        );
        assert!(counts.hints >= 1, "the E0217 hint rides along: {counts:?}");
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("E0301"), "type error reported: {out}");
        assert!(
            out.contains("E0217"),
            "outside-a-project hint reported: {out}"
        );
    }

    /// The full format still renders the ariadne report (source excerpt +
    /// header) — the machine formats must not have replaced it.
    #[test]
    fn full_still_renders_ariadne_reports() {
        let out = render(OutputFormat::Full);
        assert!(out.contains("E0301"), "code in the header: {out}");
        assert!(out.contains("main.st"), "workspace-relative path: {out}");
        assert!(out.contains("ULINT#5"), "source excerpt shown");
    }
}
