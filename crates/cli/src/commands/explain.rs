//! `rk explain <code>` — a short explanation of a diagnostic code.
//!
//! Meant for agents: the code, its title, and the explanation text, nothing
//! more — humans have the documentation site. The text comes from the same
//! committed reference the site serves (`crates/doc/diagnostics.json`,
//! generated from `crates/doc/src/examples.rs`), embedded at build time.

use serde::Deserialize;

use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};

const REFERENCE: &str = include_str!("../../../doc/diagnostics.json");

/// Unknown fields (the sources) are ignored: an agent already has its own
/// failing source. The category names the subsystem and marks `L*` as
/// advisory.
#[derive(Deserialize)]
struct Entry {
    code: String,
    category: String,
    title: String,
    description: String,
}

pub fn run_explain(code: &str, format: OutputFormat) -> CliResult<()> {
    let entries: Vec<Entry> = serde_json::from_str(REFERENCE)
        .map_err(|e| CliError::Message(format!("malformed diagnostics reference: {e}")))?;

    let needle = normalize_code(code);
    let Some(entry) = entries.into_iter().find(|e| e.code == needle) else {
        return Err(CliError::Message(format!(
            "unknown diagnostic code '{}'; codes look like E0301 or L0002",
            code.trim()
        )));
    };

    if format == OutputFormat::JsonLines {
        println!(
            "{}",
            serde_json::json!({
                "type": "explain",
                "code": entry.code,
                "category": entry.category,
                "title": entry.title,
                "description": entry.description,
            })
        );
        return Ok(());
    }
    println!("{}: {} ({})", entry.code, entry.title, entry.category);
    println!();
    println!("{}", entry.description);
    Ok(())
}

/// The code as the reference spells it, from any way a user pastes it
/// (`[E0301]` included).
fn normalize_code(code: &str) -> String {
    let code = code.trim();
    let code = code
        .strip_prefix('[')
        .and_then(|c| c.strip_suffix(']'))
        .unwrap_or(code);
    code.trim().to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever a report prints, a user can paste back, a workspace-level
    /// code (no source example) included.
    #[test]
    fn a_code_described_without_an_example_still_explains() {
        assert!(run_explain("E1401", OutputFormat::Full).is_ok());
    }

    #[test]
    fn a_code_resolves_however_it_was_pasted() {
        for spelled in ["E0301", "e0301", " E0301 ", "[E0301]", "[ e0301 ]"] {
            assert_eq!(normalize_code(spelled), "E0301", "{spelled:?}");
        }
        assert_eq!(normalize_code("[E0301"), "[E0301", "an unmatched bracket is left alone");
    }

    /// The embedded artifact must parse and cover the codes explain promises.
    #[test]
    fn reference_parses_and_knows_the_classics() {
        let entries: Vec<Entry> = serde_json::from_str(REFERENCE).expect("valid json");
        assert!(entries.len() > 200, "reference looks truncated");
        for code in ["E0301", "E0102", "E0205"] {
            let e = entries.iter().find(|e| e.code == code);
            let e = e.unwrap_or_else(|| panic!("{code} missing"));
            assert!(!e.description.is_empty(), "{code} has no explanation");
        }
    }
}
