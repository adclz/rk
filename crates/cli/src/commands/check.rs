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

/// Run diagnostics once. `Err(Failed)` if any file reported diagnostics.
fn check_once(workspace: &std::path::Path, verbose: bool, format: OutputFormat) -> CliResult<()> {
    let db = init_db(workspace, verbose, true).ok_or(CliError::Failed)?;

    let per_file = collect_diagnostics(&db, true);
    let reporter = DiagnosticReporter::new(&db, workspace).with_format(format);
    let (errors, warnings) = reporter.report_files(&per_file, &mut std::io::stderr());

    println!();
    ui::success(
        "diagnostics complete:",
        format!(
            "{} error(s), {} warning(s) found.",
            errors.red(),
            warnings.yellow()
        ),
    );

    if per_file
        .iter()
        .any(|(_, diagnostics)| !diagnostics.is_empty())
    {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}
