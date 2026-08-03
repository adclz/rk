use yansi::Paint;

use crate::cli::OutputFormat;
use crate::diagnostics::{DiagnosticReporter, collect_diagnostics};
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

/// Run diagnostics once. `Err(Failed)` — exit 1 — only when ERRORS were found:
/// warnings, infos and hints are advice, not failures (matching `rk compile`,
/// which never blocks on them). The summary still surfaces every bucket, so an
/// advice-only workspace prints its counts and exits 0 — previously it exited 1
/// while claiming "0 error(s), 0 warning(s)", which sent agents chasing nothing.
fn check_once(workspace: &std::path::Path, verbose: bool, format: OutputFormat) -> CliResult<()> {
    let db = init_db(workspace, verbose, true).ok_or(CliError::Failed)?;

    let per_file = collect_diagnostics(&db, true);
    let reporter = DiagnosticReporter::new(&db, workspace).with_format(format);
    let counts = reporter.report_files(&per_file, &mut std::io::stderr());

    let mut summary = format!(
        "{} error(s), {} warning(s)",
        counts.errors.red(),
        counts.warnings.yellow()
    );
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
