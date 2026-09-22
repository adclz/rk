use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use db::RootDatabase;
use db::workspace::Workspace;

use crate::diagnostics::DiagnosticReporter;
use crate::ui;

/// Refuse a `--workspace` that names nothing before any command looks
/// inside: one guard, one message, for all of them.
pub fn require_workspace_dir(path: &std::path::Path) -> crate::error::CliResult<()> {
    if !path.exists() {
        return Err(crate::error::CliError::msg(format!(
            "workspace path does not exist: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(crate::error::CliError::msg(format!(
            "workspace path is not a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Load and parse a workspace into a fresh [`RootDatabase`]; `None` (with
/// the reason on stderr) when the config is invalid, no `.st` files exist,
/// or, with `require_config`, there is no `config.toml`. Without a config,
/// defaults apply and no linter runs; the library still comes from
/// `RK_STDLIB_PATH`.
pub fn init_db(
    workspace: &std::path::Path,
    verbose: bool,
    require_config: bool,
) -> Option<RootDatabase> {
    let workspace_path =
        std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());

    if require_config && db::loader::resolve_config_file(workspace).is_none() {
        ui::error(format!(
            "no config.toml found in {}",
            workspace_path.display()
        ));
        return None;
    }

    if verbose {
        eprintln!("scanning workspace: {}", workspace.display());
    }

    let mut db = RootDatabase::default();
    let mut config_errors = vec![];
    let mut config_notices = vec![];

    // Configuration, then the library, then this workspace's files: the one
    // order, shared with the language server.
    let results = db::loader::open_workspace(
        &mut db,
        Some(workspace),
        PositionEncodingKind::UTF8,
        &mut config_errors,
        &mut config_notices,
    );

    // Notices are advisory and never fatal; a missing config file is not
    // relayed, the commands message it themselves.
    for notice in &config_notices {
        if !matches!(
            notice,
            db::workspace::ConfigurationNotice::ConfigFileNotFound { .. }
        ) {
            ui::error(notice);
        }
    }

    // Report config errors against the config file's own source.
    if let Some(config_file) = Workspace::try_get(&db).and_then(|w| w.config_file(&db)) {
        let config_file_url = Url::from_file_path(config_file).unwrap();
        let config_file_content = std::fs::read_to_string(config_file).unwrap();

        let reporter = DiagnosticReporter::new(&db, workspace);
        reporter.report_external(
            &config_file_url,
            &config_file_content,
            &config_errors,
            &mut std::io::stderr(),
        );

        if !config_errors.is_empty() {
            return None;
        }
    }

    if results.is_empty() {
        ui::error("no .st files found in workspace");
        return None;
    }

    // A file that cannot be read is not a file with no diagnostics: the
    // command fails, naming the file.
    let mut unreadable = false;
    for result in &results {
        match result {
            Ok(file) => {
                if verbose {
                    eprintln!("  loaded {}", file.url(&db));
                }
            }
            Err(e) => {
                ui::error(format!("failed to load {e}"));
                unreadable = true;
            }
        }
    }
    if unreadable {
        return None;
    }

    Some(db)
}
