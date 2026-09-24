use rk::commands::env::{Row, describe};

use super::{disable_stdlib, unset_stdlib};

fn find<'a>(rows: &'a [Row], key: &str) -> &'a Row {
    rows.iter().find(|row| row.key == key).expect(key)
}

/// The origin: two installs resolve differently, and nothing else says
/// which answered. This binary sits under `target/`, so the probe finds
/// the checkout's `stdlib/`.
#[test]
fn reports_the_library_it_resolved_and_where_from() {
    unset_stdlib();
    let ws = tempfile::tempdir().expect("workspace");
    let (rows, probed) = describe(ws.path());

    let stdlib = find(&rows, "stdlib");
    assert_eq!(stdlib.note.as_deref(), Some("dev-tree"), "origin is named");
    assert!(
        stdlib.value.ends_with("stdlib"),
        "the checkout's library: {}",
        stdlib.value
    );
    // The value is the path alone.
    assert!(!stdlib.value.contains('('), "no annotation in the value");
    assert!(probed.is_empty(), "nothing to list when one was found");
}

/// A disabled library is not a failure to find one, and must not be
/// reported as if the walk came up empty.
#[test]
fn a_disabled_library_is_not_a_missing_one() {
    disable_stdlib();
    let ws = tempfile::tempdir().expect("workspace");
    let (rows, probed) = describe(ws.path());

    let stdlib = find(&rows, "stdlib");
    assert!(stdlib.value.is_empty(), "no path when there is no library");
    assert_eq!(stdlib.note.as_deref(), Some("disabled"));
    assert!(probed.is_empty());
}

/// Every row is present whatever happened, so the output is a table an
/// agent can parse rather than a variable-length report.
#[test]
fn every_row_is_always_present() {
    let ws = tempfile::tempdir().expect("workspace");
    let (rows, _) = describe(ws.path());
    let keys: Vec<&str> = rows.iter().map(|row| row.key).collect();
    assert_eq!(
        keys,
        vec!["workspace", "config", "executable", "lsp", "stdlib"]
    );
}
