// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use rk::commands::env::{Row, describe};

use super::{disable_stdlib, tests_cache, unset_stdlib};

fn find<'a>(rows: &'a [Row], key: &str) -> &'a Row {
    rows.iter().find(|row| row.key == key).expect(key)
}

/// The origin: two installs resolve differently, and nothing else says
/// which answered. The standard library built in is read from its copy in
/// the cache, the folder named by its hash.
#[test]
fn reports_the_library_it_resolved_and_where_from() {
    unset_stdlib();
    let ws = tempfile::tempdir().expect("workspace");
    let rows = describe(ws.path());

    let stdlib = find(&rows, "stdlib");
    assert_eq!(stdlib.note.as_deref(), Some("bundled"), "origin is named");
    let copy = db::dependencies::cached_dir(
        &tests_cache().canonicalize().unwrap(),
        &db::stdlib::bundle().hash,
    );
    assert_eq!(stdlib.value, copy.display().to_string());
    assert!(
        copy.join("Math.st").is_file(),
        "{} holds the stdlib",
        stdlib.value
    );
}

/// A disabled library is not a failure to find one.
#[test]
fn a_disabled_library_is_not_a_missing_one() {
    disable_stdlib();
    let ws = tempfile::tempdir().expect("workspace");
    let rows = describe(ws.path());

    let stdlib = find(&rows, "stdlib");
    assert!(stdlib.value.is_empty(), "no path when there is no library");
    assert_eq!(stdlib.note.as_deref(), Some("disabled"));
}

/// A `.env` at the workspace root names the library too, and is reported
/// as the origin. Empty, it is the standard library's own workspace saying
/// it reads no copy of itself.
#[test]
fn the_env_file_of_the_workspace_names_the_library() {
    unset_stdlib();
    let lib = tempfile::tempdir().expect("library");
    let lib_root = lib.path().canonicalize().unwrap();
    let ws = tempfile::tempdir().expect("workspace");
    std::fs::write(
        ws.path().join(".env"),
        format!("RK_STDLIB_PATH=\"{}\"\n", lib_root.display()),
    )
    .unwrap();
    let rows = describe(ws.path());
    let stdlib = find(&rows, "stdlib");
    assert_eq!(stdlib.value, lib_root.display().to_string());
    assert_eq!(stdlib.note.as_deref(), Some("dotenv"));

    std::fs::write(ws.path().join(".env"), "# none\nRK_STDLIB_PATH=\n").unwrap();
    let rows = describe(ws.path());
    let stdlib = find(&rows, "stdlib");
    assert!(stdlib.value.is_empty());
    assert_eq!(stdlib.note.as_deref(), Some("disabled"));
}

/// Every row is present whatever happened, so the output is a table an
/// agent can parse rather than a variable-length report.
#[test]
fn every_row_is_always_present() {
    disable_stdlib();
    let ws = tempfile::tempdir().expect("workspace");
    let rows = describe(ws.path());
    let keys: Vec<&str> = rows.iter().map(|row| row.key).collect();
    assert_eq!(
        keys,
        vec![
            "workspace",
            "config",
            "executable",
            "lsp",
            "cache",
            "stdlib"
        ]
    );
}
