// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk env`: the paths this toolchain resolved and where each came from,
//! the way `rustc --print sysroot` or `go env GOROOT` do.

use std::path::Path;

use db::stdlib::Standard;

use crate::cli::OutputFormat;
use crate::error::CliResult;

pub fn run_env(workspace: &Path, keys: &[String], format: OutputFormat) -> CliResult<()> {
    print(&describe(workspace), keys, format)
}

/// Print the table, or the named keys alone, in `format`.
pub fn print(rows: &[Row], keys: &[String], format: OutputFormat) -> CliResult<()> {
    if keys.is_empty() && format == OutputFormat::JsonLines {
        for row in rows {
            println!("{}", env_record(row));
        }
        return Ok(());
    }

    // Named keys print the value alone, the way `go env GOROOT` does.
    if !keys.is_empty() {
        for key in keys {
            let Some(row) = rows.iter().find(|row| row.key == key) else {
                return Err(crate::error::CliError::Message(format!(
                    "unknown key '{key}' (known keys: {})",
                    rows.iter().map(|r| r.key).collect::<Vec<_>>().join(", ")
                )));
            };
            match format {
                OutputFormat::JsonLines => println!("{}", env_record(row)),
                _ => println!("{}", row.value),
            }
        }
        return Ok(());
    }

    for row in rows {
        let value = match (row.value.is_empty(), &row.note) {
            (true, Some(note)) => format!("none ({note})"),
            (true, None) => "none".to_string(),
            (false, Some(note)) => format!("{} ({note})", row.value),
            (false, None) => row.value.clone(),
        };
        println!("{:<12}{value}", row.key);
    }
    Ok(())
}

/// One line of the table: a machine-usable `value` (empty when there is none)
/// and the `note` that explains it to a human.
pub struct Row {
    pub key: &'static str,
    pub value: String,
    pub note: Option<String>,
}

pub fn row(key: &'static str, value: String) -> Row {
    Row {
        key,
        value,
        note: None,
    }
}

pub fn noted(key: &'static str, value: String, note: impl Into<String>) -> Row {
    Row {
        key,
        value,
        note: Some(note.into()),
    }
}

/// The rows, separated from printing so the resolution can be asserted.
pub fn describe(workspace: &Path) -> Vec<Row> {
    let mut rows = vec![
        row(
            "workspace",
            display_or(workspace.canonicalize().ok().as_deref()),
        ),
        row(
            "config",
            display_or(db::loader::resolve_config_file(workspace).as_deref()),
        ),
        row(
            "executable",
            display_or(std::env::current_exe().ok().as_deref()),
        ),
    ];

    let lsp = crate::spawn::lsp_binary();
    rows.push(match lsp.is_file() {
        true => row("lsp", lsp.display().to_string()),
        false => noted(
            "lsp",
            lsp.display().to_string(),
            "from PATH, not beside the executable",
        ),
    });

    rows.push(row("cache", display_or(db::cache::root().as_deref())));

    rows.push(match db::stdlib::locate(Some(workspace)) {
        Standard::Found { library, origin } => {
            noted("stdlib", library.dir.display().to_string(), origin.as_str())
        }
        Standard::Disabled => noted("stdlib", String::new(), "disabled"),
        Standard::NotAFolder(value) => noted(
            "stdlib",
            String::new(),
            format!("'{value}' is not a directory"),
        ),
        Standard::NotANamespaceName { name, .. } => noted(
            "stdlib",
            String::new(),
            format!("'{name}' is not a namespace name"),
        ),
        Standard::NotInstalled(reason) => noted("stdlib", String::new(), reason),
    });
    rows
}

fn display_or(path: Option<&Path>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "none".to_string())
}

/// One `rk env` row as a `json-lines` record.
fn env_record(row: &Row) -> String {
    serde_json::json!({ "type": "env", "key": row.key, "value": row.value, "note": row.note })
        .to_string()
}
