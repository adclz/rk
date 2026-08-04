use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use db::RootDatabase;
use db::loader::load_workspace;
use db::workspace::Workspace;

use crate::diagnostics::DiagnosticReporter;
use crate::ui;

/// Load and parse a workspace into a fresh [`RootDatabase`]. Returns `None` (and
/// reports why on stderr) if the config is invalid, the workspace holds no
/// `.st` files, or — with `require_config` — it has no `config.toml`. All
/// output goes to **stderr**: the debugger calls this while stdout is the debugger
/// transport.
///
/// Without a config the whole stack falls back to defaults: bundled stdlib,
/// default settings, and NO linter (lints need a `[linter]` section). `rk
/// check` passes `require_config = false` so a bare directory of `.st` files
/// is checkable; commands that produce artifacts keep requiring a project.
pub fn init_db(
    workspace: &std::path::Path,
    verbose: bool,
    load_stdlib: bool,
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
    let workspace_uri = std::fs::canonicalize(workspace)
        .ok()
        .and_then(|p| Url::from_file_path(p).ok());

    let mut config_errors = vec![];
    let mut config_notices = vec![];

    Workspace::init_or_update(
        &mut db,
        workspace_uri,
        PositionEncodingKind::UTF8,
        &mut config_errors,
        &mut config_notices,
    );

    if load_stdlib {
        db::loader::load_stdlib(&mut db);
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

    // Load .st files
    let results = load_workspace(&mut db, workspace);

    if results.is_empty() {
        ui::error("no .st files found in workspace");
        return None;
    }

    for result in &results {
        match result {
            Ok(file) => {
                if verbose {
                    eprintln!("  loaded {}", file.url(&db));
                }
            }
            Err(e) => ui::error(format!("failed to load: {e}")),
        }
    }

    Some(db)
}
