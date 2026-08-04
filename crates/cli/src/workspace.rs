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
/// Without a config the whole stack falls back to defaults: default settings
/// and NO linter (lints need a `[linter]` section); the library still comes
/// from `RK_STDLIB_PATH` alone. `rk check` passes `require_config = false` so
/// a bare directory of `.st` files is checkable; commands that produce
/// artifacts keep requiring a project.
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

    // Libraries always go through the loader — `resolve_library_path`
    // (RK_STDLIB_PATH) already decided whether there is anything to load.
    db::loader::load_libraries(&mut db);

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

#[cfg(test)]
mod tests {
    use super::*;
    use auto_lsp::default::db::BaseDatabase;
    use db::WorkspaceDataBase;
    use db::loader::STDLIB_PATH_ENV;

    // Environment mutation is process-global; safe here only because nextest
    // runs every test in its own process.
    fn set_env(value: &std::ffi::OsStr) {
        unsafe { std::env::set_var(STDLIB_PATH_ENV, value) };
    }
    fn unset_env() {
        unsafe { std::env::remove_var(STDLIB_PATH_ENV) };
    }

    /// A library namespace used by the tests below.
    const LIB_NS: &str = "NAMESPACE Std.S
FUNCTION pick : INT
VAR_INPUT a : INT; END_VAR
    pick := a;
END_FUNCTION
END_NAMESPACE
";

    fn write_workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, std::path::PathBuf) {
        let ws = tempfile::tempdir().expect("workspace tempdir");
        let root = std::fs::canonicalize(ws.path()).unwrap();
        std::fs::write(
            root.join("config.toml"),
            "[project]\nname = \"T\"\nversion = \"0.0\"\n",
        )
        .unwrap();
        for (name, src) in files {
            std::fs::write(root.join(name), src).unwrap();
        }
        (ws, root)
    }

    fn check(root: &std::path::Path) -> (RootDatabase, crate::diagnostics::DiagnosticCounts, String) {
        let db = init_db(root, false, true).expect("init db");
        let per_file = crate::diagnostics::collect_diagnostics(&db, false);
        let mut out = Vec::new();
        let counts = crate::diagnostics::DiagnosticReporter::new(&db, root)
            .with_format(crate::cli::OutputFormat::Concise)
            .report_files(&per_file, &mut out);
        (db, counts, String::from_utf8(out).unwrap())
    }

    /// Unset variable: no library loads, and that is not fatal — code that
    /// never touches a library is entirely unaffected.
    #[test]
    fn unset_variable_loads_no_library_and_is_not_fatal() {
        unset_env();
        let (_ws, root) = write_workspace(&[(
            "main.st",
            "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n",
        )]);
        let (db, counts, out) = check(&root);
        assert!(db.get_library_files().is_empty(), "nothing must load");
        assert!(!counts.has_errors(), "library-free code is unaffected:\n{out}");
    }

    /// A directory value loads that library; its names resolve via USING.
    #[test]
    fn variable_selects_the_library_directory() {
        let lib = tempfile::tempdir().expect("lib tempdir");
        let lib_root = std::fs::canonicalize(lib.path()).unwrap();
        std::fs::write(lib_root.join("s.st"), LIB_NS).unwrap();
        set_env(lib_root.as_os_str());

        let (_ws, root) = write_workspace(&[(
            "main.st",
            "NAMESPACE App\nUSING Std.S;\nFUNCTION main : INT\n    main := pick(1);\nEND_FUNCTION\nEND_NAMESPACE\n",
        )]);
        let (db, counts, out) = check(&root);
        assert_eq!(db.get_library_files().len(), 1, "library loaded");
        assert!(!counts.has_errors(), "library names must resolve:\n{out}");
    }

    /// The empty value is the explicit, silent "no library", how the standard
    /// library's own workspace avoids loading a second copy of itself.
    #[test]
    fn empty_variable_disables_the_library() {
        set_env(std::ffi::OsStr::new(""));
        let (_ws, root) = write_workspace(&[(
            "main.st",
            "NAMESPACE App\nUSING Std.S;\nFUNCTION main : INT\n    main := 1;\nEND_FUNCTION\nEND_NAMESPACE\n",
        )]);
        let (db, counts, _out) = check(&root);
        assert!(db.get_library_files().is_empty(), "nothing must load");
        assert!(counts.has_errors(), "USING an absent library must error");
    }

    /// A `.env` at the workspace root supplies the variable — with the usual
    /// dotenv rule: the process environment wins when both are set.
    #[test]
    fn workspace_dotenv_supplies_the_variable_and_process_env_wins() {
        let lib = tempfile::tempdir().expect("lib tempdir");
        let lib_root = std::fs::canonicalize(lib.path()).unwrap();
        std::fs::write(lib_root.join("s.st"), LIB_NS).unwrap();

        let (_ws, root) = write_workspace(&[(
            "main.st",
            "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n",
        )]);
        std::fs::write(
            root.join(".env"),
            format!("# comment\nRK_STDLIB_PATH={}\n", lib_root.display()),
        )
        .unwrap();

        unset_env();
        let (db, _, _) = check(&root);
        assert_eq!(db.get_library_files().len(), 1, ".env must be honored");

        set_env(std::ffi::OsStr::new(""));
        let (db, _, _) = check(&root);
        assert!(
            db.get_library_files().is_empty(),
            "process environment must win over .env"
        );
    }

    /// Reopening a library namespace overloads it like any other file would:
    /// the compiler makes no distinction between library and workspace
    /// declarations.
    #[test]
    fn workspace_can_overload_a_library_function() {
        let lib = tempfile::tempdir().expect("lib tempdir");
        let lib_root = std::fs::canonicalize(lib.path()).unwrap();
        std::fs::write(lib_root.join("s.st"), LIB_NS).unwrap();
        set_env(lib_root.as_os_str());

        let overload = "NAMESPACE Std.S
FUNCTION pick : STRING
VAR_INPUT a : STRING; END_VAR
    pick := a;
END_FUNCTION
END_NAMESPACE
";
        let caller = "NAMESPACE App
USING Std.S;
FUNCTION main : INT
VAR s : STRING; END_VAR
    s := pick('x');
    main := pick(1);
END_FUNCTION
END_NAMESPACE
";
        let (_ws, root) = write_workspace(&[("overload.st", overload), ("main.st", caller)]);
        let (_db, counts, out) = check(&root);
        assert!(
            !counts.has_errors(),
            "both overloads must resolve by signature:\n{out}"
        );
    }
}
