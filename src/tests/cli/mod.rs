mod args;
mod diagnostics;
mod duration;
mod env;
mod file_order;
mod fmt;
mod report;
mod unit_assertions;
mod watcher;
mod workspace;

use db::loader::STDLIB_PATH_ENV;

/// A workspace on disk: a `config.toml` and `files`, as `(name, source)`.
/// A `config.toml` among `files` replaces the default one. The directory
/// lives as long as the guard does; the path is canonical,
/// which is what `rk` resolves a workspace to.
fn temp_workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, std::path::PathBuf) {
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

// Environment mutation is process-global; safe here only because nextest
// runs every test in its own process.

/// No standard library at all, so a test does not inherit the developer's.
/// Distinct from [`unset_stdlib`]: an unset variable falls through to the
/// probe beside the executable.
fn disable_stdlib() {
    unsafe { std::env::set_var(STDLIB_PATH_ENV, "") };
}

fn unset_stdlib() {
    unsafe { std::env::remove_var(STDLIB_PATH_ENV) };
}

fn set_stdlib(path: &std::ffi::OsStr) {
    unsafe { std::env::set_var(STDLIB_PATH_ENV, path) };
}
