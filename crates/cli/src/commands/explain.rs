//! `rk explain <code>` — a short explanation of a diagnostic code.
//!
//! Meant for agents: the code, its title, and the explanation text, nothing
//! more — humans have the documentation site. The text comes from the same
//! committed reference the site serves
//! (`docs/static/diagnostics/diagnostics.json`, generated from
//! `crates/doc/src/examples.rs`), embedded at build time.

use serde::Deserialize;

use crate::error::{CliError, CliResult};

const REFERENCE: &str = include_str!("../../../../docs/static/diagnostics/diagnostics.json");

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

pub fn run_explain(code: &str) -> CliResult<()> {
    let entries: Vec<Entry> = serde_json::from_str(REFERENCE)
        .map_err(|e| CliError::Message(format!("malformed diagnostics reference: {e}")))?;

    let needle = code.trim().to_ascii_uppercase();
    let Some(entry) = entries.into_iter().find(|e| e.code == needle) else {
        return Err(CliError::Message(format!(
            "unknown diagnostic code '{}'; codes look like E0301 or L0002",
            code.trim()
        )));
    };

    println!("{}: {} ({})", entry.code, entry.title, entry.category);
    println!();
    println!("{}", entry.description);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded artifact must parse and cover the codes explain promises.
    #[test]
    fn reference_parses_and_knows_the_classics() {
        let entries: Vec<Entry> = serde_json::from_str(REFERENCE).expect("valid json");
        assert!(entries.len() > 200, "reference looks truncated");
        for code in ["E0301", "E0101", "E0225"] {
            let e = entries.iter().find(|e| e.code == code);
            let e = e.unwrap_or_else(|| panic!("{code} missing"));
            assert!(!e.description.is_empty(), "{code} has no explanation");
        }
    }
}
