use auto_lsp::default::db::BaseDatabase;
use yansi::Paint;

use crate::diagnostics::report_diagnostics;
use crate::workspace::init_db;

pub fn run_check(workspace: &std::path::Path, verbose: bool) {
    let Some(db) = init_db(workspace, verbose, true) else {
        return;
    };

    let workspace_path = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let config = ariadne::Config::new().with_color(true).with_tab_width(2);

    let caches = db
        .get_files()
        .iter()
        .map(|file| (file.url(&db).as_str(), file.document(&db).as_str()))
        .collect::<Vec<(&str, &str)>>();

    let linter_config = db::config_file::get_config(&db).linter.clone();

    let mut _has_errors = false;
    let mut total_errors = 0;
    let mut total_warnings = 0;

    for file in db.get_files().iter() {
        let mut diagnostics = hir::check::diagnostics_for_file(&db, *file)
            .as_ref()
            .clone();
        if let Some(ref linter_config) = linter_config {
            linter::lint_file(&db, *file, linter_config, &mut diagnostics);
        }

        if !diagnostics.is_empty() {
            _has_errors = true;

            report_diagnostics(
                &db,
                &workspace_path,
                config,
                file.url(&db),
                &file.document(&db).texter.text,
                &diagnostics,
                &caches,
                &mut total_errors,
                &mut total_warnings,
            );
        }
    }

    println!(
        "\n{}{} error(s), {} warning(s) found.",
        "diagnostics complete: ".bold().bright_green(),
        total_errors.to_string().fg(ariadne::Color::Red),
        total_warnings.to_string().fg(ariadne::Color::Yellow)
    );

    if _has_errors {
        std::process::exit(1);
    }
}
