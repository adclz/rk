//! `rk env` — the paths this toolchain resolved, and where each came from.
//!
//! Every toolchain that finds its library relative to itself exposes the
//! answer (`rustc --print sysroot`, `zig env`, `go env GOROOT`), for one
//! reason: resolution that succeeds silently is impossible to debug when it
//! succeeds WRONGLY. A user with two installs, or a copy whose library was
//! only half written, sees a working command and a wrong answer. Without this
//! the only way to ask is to read the source and redo the walk by hand.

use std::path::Path;

use db::loader::{LibraryPathResolution, resolve_library_path};

use crate::error::CliResult;

pub fn run_env(workspace: &Path, keys: &[String]) -> CliResult<()> {
    let (rows, probed) = describe(workspace);

    // Named keys print the value alone, the way `go env GOROOT` does.
    if !keys.is_empty() {
        for key in keys {
            let Some(row) = rows.iter().find(|row| row.key == key) else {
                return Err(crate::error::CliError::Message(format!(
                    "unknown key '{key}'; known keys: {}",
                    rows.iter().map(|r| r.key).collect::<Vec<_>>().join(", ")
                )));
            };
            println!("{}", row.value);
        }
        return Ok(());
    }

    for row in &rows {
        let value = match (row.value.is_empty(), &row.note) {
            (true, Some(note)) => format!("none ({note})"),
            (true, None) => "none".to_string(),
            (false, Some(note)) => format!("{} ({note})", row.value),
            (false, None) => row.value.clone(),
        };
        println!("{:<12}{value}", row.key);
    }
    for path in probed {
        crate::ui::detail(format!("    looked in {}", path.display()));
    }
    Ok(())
}

/// One line of the table: a machine-usable `value` (empty when there is none)
/// and the `note` that explains it to a human.
struct Row {
    key: &'static str,
    value: String,
    note: Option<String>,
}

fn row(key: &'static str, value: String) -> Row {
    Row {
        key,
        value,
        note: None,
    }
}

fn noted(key: &'static str, value: String, note: impl Into<String>) -> Row {
    Row {
        key,
        value,
        note: Some(note.into()),
    }
}

/// The rows, and the paths to list when no library was found; separated
/// from printing so the resolution can be asserted.
fn describe(workspace: &Path) -> (Vec<Row>, Vec<std::path::PathBuf>) {
    let mut rows = vec![
        row("workspace", display_or(workspace.canonicalize().ok().as_deref())),
        row(
            "config",
            display_or(db::loader::resolve_config_file(workspace).as_deref()),
        ),
        row("executable", display_or(std::env::current_exe().ok().as_deref())),
    ];

    let runtime = crate::spawn::runtime_binary();
    rows.push(match runtime.is_file() {
        true => row("runtime", runtime.display().to_string()),
        // Not an error here: only a command that needs to RUN something can
        // say whether this matters.
        false => noted(
            "runtime",
            runtime.display().to_string(),
            "not beside the executable; from PATH",
        ),
    });

    rows.push(row("home", display_or(crate::home::rk_home().ok().as_deref())));

    // The library last: it is the one with a story, and the probed paths that
    // follow it would otherwise split the table in two.
    let mut probed = Vec::new();
    rows.push(match resolve_library_path(Some(workspace)) {
        LibraryPathResolution::Found { dir, origin } => {
            noted("stdlib", dir.display().to_string(), origin.as_str())
        }
        LibraryPathResolution::Disabled => noted("stdlib", String::new(), "disabled"),
        LibraryPathResolution::Invalid(value) => noted(
            "stdlib",
            String::new(),
            format!("'{value}' is not a directory"),
        ),
        LibraryPathResolution::NotFound { probed: paths } => {
            probed = paths;
            noted("stdlib", String::new(), "not found")
        }
    });

    (rows, probed)
}

fn display_or(path: Option<&Path>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(rows: &'a [Row], key: &str) -> &'a Row {
        rows.iter().find(|row| row.key == key).expect(key)
    }

    /// The origin: two installs resolve differently, and nothing else says
    /// which answered. This binary sits under `target/`, so the probe finds
    /// the checkout's `stdlib/`.
    #[test]
    fn reports_the_library_it_resolved_and_where_from() {
        unsafe { std::env::remove_var(db::loader::STDLIB_PATH_ENV) };
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
        unsafe { std::env::set_var(db::loader::STDLIB_PATH_ENV, "") };
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
            vec!["workspace", "config", "executable", "runtime", "home", "stdlib"]
        );
    }
}
