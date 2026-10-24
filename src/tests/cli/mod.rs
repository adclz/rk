// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

mod args;
mod backtrace;
mod diagnostics;
mod duration;
mod env;
mod file_order;
mod fmt;
mod front_end;
mod memory;
mod nesting;
mod release;
mod report;
mod stack_size;
mod test_filter;
mod unit_assertions;
mod validation;
mod watcher;
mod workspace;

use db::stdlib::PATH_ENV;

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
/// Distinct from [`unset_stdlib`]: an unset variable reads the one built
/// in.
fn disable_stdlib() {
    unsafe { std::env::set_var(PATH_ENV, "") };
}

/// The standard library built into the compiler, written in a cache the
/// tests share rather than in the developer's.
fn unset_stdlib() {
    unsafe {
        std::env::remove_var(PATH_ENV);
        std::env::set_var(db::cache::CACHE_DIR_ENV, tests_cache());
    }
}

fn set_stdlib(path: &std::ffi::OsStr) {
    unsafe { std::env::set_var(PATH_ENV, path) };
}

/// The cache the tests write the standard library in. They share it, as
/// builds share a user's cache.
fn tests_cache() -> std::path::PathBuf {
    std::env::temp_dir().join("rk-tests-cache")
}
