use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use db::RootDatabase;
use db::loader::load_workspace;
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

#[cfg(test)]
mod tests {
    use super::*;
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
    /// The explicit "no library", distinct from [`unset_env`]: an unset
    /// variable falls through to the probe.
    fn disable_env() {
        unsafe { std::env::set_var(STDLIB_PATH_ENV, "") };
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

    /// `MAIN.ST` is a source file: discovery matches the extension
    /// case-insensitively.
    #[test]
    fn an_uppercase_extension_is_a_source_file() {
        disable_env();
        let (_ws, root) = write_workspace(&[(
            "MAIN.ST",
            "FUNCTION f : INT\n    f := not_declared;\nEND_FUNCTION\n",
        )]);
        let (_db, counts, out) = check(&root);
        assert!(counts.has_errors(), "the file was checked and its error reported:\n{out}");
        assert!(out.contains("not_declared"), "{out}");
    }

    #[test]
    fn a_workspace_path_must_be_an_existing_directory() {
        let ws = tempfile::tempdir().unwrap();
        assert!(require_workspace_dir(ws.path()).is_ok());

        let text = |r: crate::error::CliResult<()>| match r {
            Err(crate::error::CliError::Message(m)) => m,
            other => panic!("expected a message, got {other:?}"),
        };
        let missing = ws.path().join("definitely").join("not").join("here");
        let err = text(require_workspace_dir(&missing));
        assert!(err.contains("does not exist"), "{err}");
        // The message prints the path with the platform's separators.
        assert!(
            err.contains(&missing.display().to_string()),
            "names the path: {err}"
        );

        let file = ws.path().join("main.st");
        std::fs::write(&file, "").unwrap();
        let err = text(require_workspace_dir(&file));
        assert!(err.contains("not a directory"), "{err}");
    }

    /// A file the loader cannot decode fails the whole command.
    #[test]
    fn an_unreadable_file_fails_the_load() {
        disable_env();
        let (_ws, root) = write_workspace(&[(
            "good.st",
            "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n",
        )]);
        // One byte that is not UTF-8, inside a comment; the rest is fine.
        std::fs::write(
            root.join("bad.st"),
            b"FUNCTION g : INT\n    g := 2; (* caf\xe9 *)\nEND_FUNCTION\n",
        )
        .unwrap();
        assert!(
            init_db(&root, false, true).is_none(),
            "an unreadable file must fail the load, not vanish from it"
        );
    }

    /// The failure names the file: a workspace of many files cannot otherwise
    /// tell which one it could not read.
    #[test]
    fn a_load_failure_names_the_file() {
        let (_ws, root) = write_workspace(&[]);
        std::fs::write(root.join("bad.st"), b"FUNCTION g : INT (* \xff *)\nEND_FUNCTION\n")
            .unwrap();
        let mut db = RootDatabase::default();
        let results = db::loader::load_workspace(&mut db, &root);
        let err = results
            .iter()
            .find_map(|r| r.as_ref().err())
            .expect("the undecodable file is reported");
        assert!(
            err.contains("bad.st"),
            "the error must carry the path, got: {err}"
        );
    }

    /// The control: the same file with the byte removed loads and checks.
    #[test]
    fn the_same_file_loads_once_it_is_readable() {
        disable_env();
        let (_ws, root) = write_workspace(&[(
            "bad.st",
            "FUNCTION g : INT\n    g := 2; (* cafe *)\nEND_FUNCTION\n",
        )]);
        let (_db, counts, out) = check(&root);
        assert!(!counts.has_errors(), "clean once readable:\n{out}");
    }

    /// Unset variable: the library is found beside the executable, which for
    /// this test binary is the checkout's own `stdlib/`.
    #[test]
    fn unset_variable_finds_the_library_beside_the_executable() {
        unset_env();
        let (_ws, root) = write_workspace(&[(
            "main.st",
            "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n",
        )]);
        let (db, counts, out) = check(&root);
        assert!(
            !db.get_library_files().is_empty(),
            "the probe must reach the checkout's stdlib"
        );
        assert!(!counts.has_errors(), "the library must compile clean:\n{out}");
    }

    /// The explicit "no library" still loads none, and that is not fatal —
    /// code that never touches a library is entirely unaffected.
    #[test]
    fn disabled_variable_loads_no_library_and_is_not_fatal() {
        disable_env();
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

    /// A broken library refuses to build, visibly; `rk check` stays
    /// workspace-only.
    #[test]
    fn library_errors_gate_compilation() {
        let lib = tempfile::tempdir().expect("lib tempdir");
        let lib_root = std::fs::canonicalize(lib.path()).unwrap();
        std::fs::write(
            lib_root.join("s.st"),
            "NAMESPACE Std.S
FUNCTION broken : INT
    broken := 'not an int';
END_FUNCTION
END_NAMESPACE
",
        )
        .unwrap();
        set_env(lib_root.as_os_str());

        let (_ws, root) = write_workspace(&[(
            "main.st",
            "FUNCTION main : INT
    main := 1;
END_FUNCTION
",
        )]);
        let (db, counts, _out) = check(&root);
        assert!(!counts.has_errors(), "the check surface stays workspace-only");

        let err = crate::compiler::build_core_profile(
            &db,
            &root,
            false,
            crate::cli::OutputFormat::Full,
            wasm_codegen::Profile::Debug,
        )
        .expect_err("a broken library must refuse to compile");
        assert!(
            err.contains("library files"),
            "the refusal names the library as the cause:
{err}"
        );
        assert!(
            err.contains("E0308"),
            "and carries the library's own diagnostics:
{err}"
        );
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

    /// Only the workspace's tests enter the manifest: a library is compiled
    /// like everything else, but its tests are not this workspace's to run.
    #[test]
    fn library_tests_stay_out_of_the_manifest() {
        let lib = tempfile::tempdir().expect("lib tempdir");
        let lib_root = std::fs::canonicalize(lib.path()).unwrap();
        std::fs::write(
            lib_root.join("s.st"),
            "NAMESPACE Std.S\n{test}\nFUNCTION test_lib : BOOL\n    test_lib := TRUE;\nEND_FUNCTION\nEND_NAMESPACE\n",
        )
        .unwrap();
        set_env(lib_root.as_os_str());

        let (_ws, root) = write_workspace(&[(
            "main.st",
            "{test}\nFUNCTION test_mine : BOOL\n    test_mine := TRUE;\nEND_FUNCTION\n",
        )]);
        let (db, counts, out) = check(&root);
        assert!(!counts.has_errors(), "precondition:\n{out}");

        let sem_indices: Vec<_> = crate::file_order::ordered_files_with_libraries(&db)
            .into_iter()
            .map(|file| hir::hir_def::semantic_index::semantic_index(&db, file))
            .collect();
        let module = mir::lower::lower_module::lower_modules(&db, &sem_indices)
            .expect("lowering must succeed");
        let names: Vec<_> = module
            .test_manifest
            .tests
            .iter()
            .map(|t| t.path.as_str())
            .collect();
        assert_eq!(names, ["test_mine"], "only the workspace test runs");
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
