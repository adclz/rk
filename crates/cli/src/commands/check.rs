// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use yansi::Paint;

use crate::cli::OutputFormat;
use crate::diagnostics::{DiagnosticReporter, collect_diagnostics, collect_library_errors};
use crate::error::{CliError, CliResult};
use crate::ui;
use crate::workspace::init_db;

pub fn run_check(
    workspace: &std::path::Path,
    watch: bool,
    verbose: bool,
    format: OutputFormat,
) -> CliResult<()> {
    if watch {
        crate::watcher::watch_and_run(workspace, || {
            let _ = check_once(workspace, verbose, format);
        });
        Ok(())
    } else {
        check_once(workspace, verbose, format)
    }
}

/// Run diagnostics once. `Err(Failed)` (exit 1) only when errors were
/// found: warnings, infos and hints are advice.
fn check_once(workspace: &std::path::Path, verbose: bool, format: OutputFormat) -> CliResult<()> {
    // A bare directory of .st files is checkable without a config.toml.
    // Stderr stays a pure diagnostics stream.
    if db::loader::resolve_config_file(workspace).is_none() {
        ui::detail("no config.toml found: checking with defaults");
    }
    let db = init_db(workspace, verbose, false).ok_or(CliError::Failed)?;

    let per_file = collect_diagnostics(&db, true);
    let reporter = DiagnosticReporter::new(&db, workspace).with_format(format);
    let mut counts = reporter.report_files(&per_file, &mut std::io::stderr());
    // A library that does not check does not build either: its errors are
    // the program's to see before `rk compile` refuses it. Its errors only,
    // as the build reports them: its warnings are its author's.
    let in_libraries = reporter
        .report_files(&collect_library_errors(&db), &mut std::io::stderr())
        .errors;
    counts.errors += in_libraries;

    let mut summary = format!("{} error(s)", counts.errors.red());
    if in_libraries > 0 {
        summary.push_str(&format!(" ({in_libraries} in library files)"));
    }
    summary.push_str(&format!(", {} warning(s)", counts.warnings.yellow()));
    if counts.infos > 0 {
        summary.push_str(&format!(", {} info(s)", counts.infos.blue()));
    }
    if counts.hints > 0 {
        summary.push_str(&format!(", {} hint(s)", counts.hints.blue()));
    }
    summary.push_str(" found.");

    println!();
    ui::success("diagnostics complete:", summary);

    if counts.has_errors() {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
