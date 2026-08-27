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

pub fn run_env(workspace: &Path) -> CliResult<()> {
    let (rows, probed) = describe(workspace);
    for (key, value) in rows {
        println!("{key:<12}{value}");
    }
    for path in probed {
        crate::ui::detail(format!("    looked in {}", path.display()));
    }
    Ok(())
}

/// The rows, and the paths to list when no library was found; separated
/// from printing so the resolution can be asserted.
fn describe(workspace: &Path) -> (Vec<(&'static str, String)>, Vec<std::path::PathBuf>) {
    let mut rows = vec![
        (
            "workspace",
            display_or(workspace.canonicalize().ok().as_deref()),
        ),
        (
            "config",
            display_or(db::loader::resolve_config_file(workspace).as_deref()),
        ),
        (
            "executable",
            display_or(std::env::current_exe().ok().as_deref()),
        ),
    ];

    let runtime = crate::spawn::runtime_binary();
    rows.push((
        "runtime",
        match runtime.is_file() {
            true => runtime.display().to_string(),
            // Not an error here: only a command that needs to RUN something
            // can say whether this matters.
            false => format!(
                "{} (not beside the executable; from PATH)",
                runtime.display()
            ),
        },
    ));

    rows.push(("home", display_or(crate::home::rk_home().ok().as_deref())));

    // The library last: it is the one with a story, and the probed paths that
    // follow it would otherwise split the table in two.
    let mut probed = Vec::new();
    let library = match resolve_library_path(Some(workspace)) {
        LibraryPathResolution::Found { dir, origin } => {
            format!("{} ({})", dir.display(), origin.as_str())
        }
        LibraryPathResolution::Disabled => "none (disabled)".to_string(),
        LibraryPathResolution::Invalid(value) => {
            format!("none ('{value}' is not a directory)")
        }
        LibraryPathResolution::NotFound { probed: paths } => {
            probed = paths;
            "none (not found)".to_string()
        }
    };
    rows.push(("stdlib", library));

    (rows, probed)
}

fn display_or(path: Option<&Path>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(rows: &'a [(&str, String)], key: &str) -> &'a str {
        &rows.iter().find(|(k, _)| *k == key).expect(key).1
    }

    /// The origin: two installs resolve differently, and nothing else says
    /// which answered. This binary sits under `target/`, so the probe finds
    /// the checkout's `stdlib/`.
    #[test]
    fn reports_the_library_it_resolved_and_where_from() {
        unsafe { std::env::remove_var(db::loader::STDLIB_PATH_ENV) };
        let ws = tempfile::tempdir().expect("workspace");
        let (rows, probed) = describe(ws.path());

        let stdlib = value(&rows, "stdlib");
        assert!(stdlib.ends_with("(dev-tree)"), "origin is named: {stdlib}");
        assert!(stdlib.contains("stdlib"), "the checkout's library: {stdlib}");
        assert!(probed.is_empty(), "nothing to list when one was found");
    }

    /// A disabled library is not a failure to find one, and must not be
    /// reported as if the walk came up empty.
    #[test]
    fn a_disabled_library_is_not_a_missing_one() {
        unsafe { std::env::set_var(db::loader::STDLIB_PATH_ENV, "") };
        let ws = tempfile::tempdir().expect("workspace");
        let (rows, probed) = describe(ws.path());

        assert_eq!(value(&rows, "stdlib"), "none (disabled)");
        assert!(probed.is_empty());
    }

    /// Every row is present whatever happened, so the output is a table an
    /// agent can parse rather than a variable-length report.
    #[test]
    fn every_row_is_always_present() {
        let ws = tempfile::tempdir().expect("workspace");
        let (rows, _) = describe(ws.path());
        let keys: Vec<&str> = rows.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec!["workspace", "config", "executable", "runtime", "home", "stdlib"]
        );
    }
}
