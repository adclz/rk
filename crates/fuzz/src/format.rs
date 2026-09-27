//! The formatter, as `rk fmt` and the editor's format-on-save run it.
//!
//! Idempotence alone proves little: `BOOL READ_ONLY` once formatted to
//! `BOOLREAD_ONLY`, stably (src/tests/lsp/formatter_preserves_meaning.rs).
//! So the output must also parse and mean what the input meant.

use auto_lsp::tree_sitter::Parser;

use crate::{Finding, session};

pub fn check(source: &str) -> Result<(), Finding> {
    let once = match formatter::format_source(source) {
        Ok(text) => text,
        // Refusing a file that does not parse is the contract.
        Err(_) if !parses(source) => return Ok(()),
        Err(e) => {
            return Err(Finding::new(
                "format-refused",
                format!("a source that parses was not formatted: {e}"),
            ));
        }
    };

    let twice = formatter::format_source(&once).map_err(|e| {
        Finding::new(
            "format-unparsable",
            format!("the formatter's own output does not format: {e}"),
        )
    })?;
    // Past the formatter's own gate: only now is a full check worth it.
    let Some(before) = Analysis::of(source) else {
        return Ok(());
    };
    if twice != once {
        // The grammar recovers some mistakes (a missing `;`) without an
        // ERROR node, and the formatter goes on to format them. A bug
        // there is real but minor, and kept apart so it does not bury one
        // on a program that compiles.
        let oracle = match before.accepted {
            true => "format-idempotence",
            false => "format-idempotence-rejected",
        };
        return Err(Finding::new(
            oracle,
            format!(
                "formatting twice changes the text{}",
                first_difference(&once, &twice)
            ),
        ));
    }

    // Meaning is only defined for a program `rk check` accepts: on a
    // rejected one, completing the missing `;` is the formatter's job.
    if !before.accepted {
        return Ok(());
    }
    let Some(after) = Analysis::of(&once) else {
        return Ok(());
    };
    if before.diagnostics != after.diagnostics {
        return Err(Finding::new(
            "format-meaning",
            format!(
                "formatting changed the diagnostics of an accepted program:\n  before: {:?}\n  after:  {:?}",
                before.diagnostics, after.diagnostics
            ),
        ));
    }
    Ok(())
}

fn parses(source: &str) -> bool {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rk::LANGUAGE.into())
        .expect("the rk grammar loads");
    parser
        .parse(source, None)
        .is_some_and(|tree| !tree.root_node().has_error())
}

/// What `rk check` says of a text, in a database of its own: registering
/// both texts in one would make every POU a duplicate of itself.
struct Analysis {
    accepted: bool,
    diagnostics: Vec<String>,
}

impl Analysis {
    fn of(source: &str) -> Option<Self> {
        let (db, file) = session::load(source)?;
        Some(Self {
            accepted: session::compiles(&db, file),
            diagnostics: session::diagnostic_identities(&db, file),
        })
    }
}

/// The first line that differs, with the one after it on each side: a
/// comment that moved to the next line reads as moved, not as lost.
fn first_difference(a: &str, b: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let Some(n) = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) else {
        return " (line endings)".to_string();
    };
    let near = |lines: &[&str]| {
        lines
            .iter()
            .skip(n)
            .take(2)
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
    };
    format!(
        " at line {}:\n  once:  {:?}\n  twice: {:?}",
        n + 1,
        near(&a),
        near(&b)
    )
}
