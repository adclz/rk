use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use db::RootDatabase;
use db::loader::load_workspace;
use db::workspace::Workspace;
use yansi::Paint;

use crate::diagnostics::report_diagnostics;

pub fn init_db(workspace: &std::path::Path, verbose: bool, load_stdlib: bool) -> Option<RootDatabase> {
    let workspace_path = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());

    if db::loader::resolve_config_file(workspace).is_none() {
        eprintln!(
            "{}no config.toml found in {}",
            "Error: ".red(),
            workspace_path.display()
        );
        return None;
    }

    if verbose {
        println!("scanning workspace: {}", workspace.display());
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

    // Report config errors
    let config = ariadne::Config::new().with_color(true).with_tab_width(2);
    if let Some(config_file) = Workspace::try_get(&db).and_then(|w| w.config_file(&db)) {
        let config_file_url = Url::from_file_path(config_file).unwrap();
        let config_file_content = std::fs::read_to_string(config_file).unwrap();
        let caches = vec![(config_file_url.as_str(), config_file_content.as_str())];

        let mut total_errors = 0;
        let mut total_warnings = 0;

        report_diagnostics(
            &db,
            &workspace_path,
            config,
            &config_file_url,
            &config_file_content,
            &config_errors,
            &caches,
            &mut total_errors,
            &mut total_warnings,
        );

        if !config_errors.is_empty() {
            return None;
        }
    }

    // Load .st files
    let results = load_workspace(&mut db, workspace);

    if results.is_empty() {
        println!("no .st files found in workspace");
        return None;
    }

    for result in &results {
        match result {
            Ok(file) => {
                if verbose {
                    println!("  loaded {}", file.url(&db));
                }
            }
            Err(e) => {
                println!("  failed to load: {}", e);
            }
        }
    }

    Some(db)
}
