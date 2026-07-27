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
    let linter_config = with_linter
        .then(|| db::config_file::get_config(db).linter.clone())
        .flatten();

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

/// Renders collected diagnostics with ariadne (color, 2-space tabs) and tallies
/// error/warning counts, shortening paths to be workspace-relative.
pub struct DiagnosticReporter<'db> {
    db: &'db RootDatabase,
    workspace_path: PathBuf,
    config: ariadne::Config,
}

impl<'db> DiagnosticReporter<'db> {
    pub fn new(db: &'db RootDatabase, workspace: &Path) -> Self {
        let workspace_path =
            std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
        let config = ariadne::Config::new().with_color(true).with_tab_width(2);
        Self {
            db,
            workspace_path,
            config,
        }
    }

    /// Render every file's diagnostics to `out`, returning `(errors, warnings)`.
    /// The source cache spans all workspace files so cross-file related info
    /// renders.
    pub fn report_files(
        &self,
        per_file: &[(File, Vec<IdeDiagnostic>)],
        out: &mut dyn Write,
    ) -> (i32, i32) {
        let caches: Vec<(&str, &str)> = crate::file_order::ordered_files(self.db)
            .into_iter()
            .map(|file| (file.url(self.db).as_str(), file.document(self.db).as_str()))
            .collect();

        let (mut errors, mut warnings) = (0, 0);
        for (file, diagnostics) in per_file {
            if diagnostics.is_empty() {
                continue;
            }
            let (e, w) = self.render(
                file.url(self.db),
                &file.document(self.db).texter.text,
                diagnostics,
                &caches,
                out,
            );
            errors += e;
            warnings += w;
        }
        (errors, warnings)
    }

    /// Render a single external source's diagnostics (e.g. config-file errors,
    /// which aren't a workspace [`File`]), returning `(errors, warnings)`.
    pub fn report_external(
        &self,
        url: &Url,
        content: &str,
        diagnostics: &[IdeDiagnostic],
        out: &mut dyn Write,
    ) -> (i32, i32) {
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
    ) -> (i32, i32) {
        let url_str = url.as_str();
        let rel_path = url
            .to_file_path()
            .ok()
            .and_then(|abs_path| {
                abs_path
                    .strip_prefix(&self.workspace_path)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| url_str.to_string());

        let (mut errors, mut warnings) = (0, 0);
        for diagnostic in diagnostics {
            match diagnostic.diagnostic.severity {
                Some(DiagnosticSeverity::ERROR) => errors += 1,
                Some(DiagnosticSeverity::WARNING) => warnings += 1,
                _ => {}
            }

            let report = diagnostic.create_report(self.db, url, content, Some(self.config), true);
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
        (errors, warnings)
    }
}
