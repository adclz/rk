//! The generator refuses to emit a reference that disagrees with the compiler.
//!
//! `crates/doc/diagnostics.json` is not only the documentation
//! site's data — `rk explain` embeds it at build time, so a wrong entry is a
//! wrong answer given to a user, not merely a stale page. Every check here
//! therefore runs before anything is written: a failing run leaves the
//! committed artifact untouched rather than replacing it with a bad one.
//!
//! Four ways the reference and the compiler can disagree:
//!
//! 1. a documented code the compiler does not define (a ghost page),
//! 2. a defined code with no example (an unanswerable `rk explain`),
//! 3. an example that produces some *other* diagnostic (a wrong answer),
//! 4. an example that produces nothing at all — which used to drop the entry
//!    from the JSON silently, so `rk explain` denied a code the compiler
//!    really does emit.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::examples::ErrorExample;

/// Codes that exist in the compiler but have no runnable example: their file
/// in `crates/doc/examples/` carries no fence. This list may only SHRINK:
/// give a code an example, remove it from here. A new code must ship with its
/// example — adding to this list defeats the guard, and the gap it papers
/// over becomes the 35-ghost / 27-missing drift the April 2026 docs
/// accumulated.
pub const KNOWN_UNDOCUMENTED: &[&str] = &[
    // Workspace-level: fires when the workspace has no config file, so no
    // SOURCE example can trigger it in the doc harness.
    "E1401",
];

/// The crates whose sources define diagnostic codes.
fn code_defining_crates() -> Vec<PathBuf> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ is the parent of crates/doc");
    vec![
        crates.join("hir").join("src"),
        crates.join("linter").join("src"),
    ]
}

/// Every `"EXXXX"` / `"LXXXX"` string literal in a source tree — the
/// diagnostic codes a crate defines (each `code()` impl returns one).
fn codes_in(dir: &Path, out: &mut BTreeSet<String>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read source dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read source");
            let bytes = text.as_bytes();
            for i in 0..bytes.len().saturating_sub(7) {
                if bytes[i] == b'"'
                    && (bytes[i + 1] == b'E' || bytes[i + 1] == b'L')
                    && bytes[i + 2..i + 6].iter().all(u8::is_ascii_digit)
                    && bytes[i + 6] == b'"'
                {
                    out.insert(text[i + 1..i + 6].to_string());
                }
            }
        }
    }
}

/// The codes the compiler actually defines.
pub fn defined_codes() -> BTreeSet<String> {
    let mut defined = BTreeSet::new();
    for dir in code_defining_crates() {
        codes_in(&dir, &mut defined);
    }
    defined
}

/// The diagnostic codes appearing in a rendered report, as `[E0301]` markers.
pub fn codes_in_output(output: &str) -> BTreeSet<String> {
    output
        .match_indices('[')
        .filter_map(|(i, _)| output.get(i + 1..i + 6))
        .filter(|c| {
            let b = c.as_bytes();
            b.len() == 5 && (b[0] == b'E' || b[0] == b'L') && b[1..].iter().all(u8::is_ascii_digit)
        })
        .map(str::to_string)
        .collect()
}

/// Every disagreement between the examples and the compiler, as lines ready
/// to print. Empty means the reference may be written.
pub fn problems(examples: &[ErrorExample], produced: &[(&str, BTreeSet<String>)]) -> Vec<String> {
    let mut problems = Vec::new();

    let defined = defined_codes();
    if defined.len() < 100 {
        problems.push(format!(
            "the source scan found implausibly few codes ({}) — did the error \
             definitions move out of crates/hir + crates/linter?",
            defined.len()
        ));
        return problems;
    }

    let documented: BTreeSet<String> = examples.iter().map(|e| e.code.to_string()).collect();
    // Documented with a runnable example. A code that fires at the workspace
    // level is described without one, and stays on the allowlist: `rk explain`
    // used to deny such a code exists.
    let exemplified: BTreeSet<String> = examples
        .iter()
        .filter(|e| !e.sources.is_empty())
        .map(|e| e.code.to_string())
        .collect();
    let debt: BTreeSet<String> = KNOWN_UNDOCUMENTED.iter().map(|s| s.to_string()).collect();

    for ghost in documented.difference(&defined) {
        problems.push(format!(
            "{ghost}: documented, but the compiler defines no such code — delete the example"
        ));
    }
    for missing in defined
        .difference(&documented)
        .filter(|c| !debt.contains(*c))
    {
        problems.push(format!(
            "{missing}: defined by the compiler with no example — write \
             `crates/doc/examples/{missing}.md` (do NOT grow KNOWN_UNDOCUMENTED)"
        ));
    }
    for paid in debt.intersection(&exemplified) {
        problems.push(format!(
            "{paid}: now has an example — remove it from KNOWN_UNDOCUMENTED"
        ));
    }
    for gone in debt.difference(&defined) {
        problems.push(format!(
            "{gone}: allowlisted but no longer defined — remove it from KNOWN_UNDOCUMENTED"
        ));
    }

    for (code, got) in produced {
        let want = *code;
        if got.contains(want) {
            // The reader must see the documented code alone: a companion error
            // fires first, reads as the point of the example, and buries it.
            let others: Vec<&str> = got
                .iter()
                .map(String::as_str)
                .filter(|c| *c != want)
                .collect();
            if !others.is_empty() {
                problems.push(format!(
                    "{code}: its example also produces {} — rewrite the source so only {want} fires",
                    others.join(", ")
                ));
            }
            continue;
        }
        if got.is_empty() {
            problems.push(format!(
                "{code}: its example produces NO diagnostic at all — the code would vanish from \
                 the reference and `rk explain {want}` would deny it exists"
            ));
        } else {
            problems.push(format!(
                "{code}: its example produces {} instead — `rk explain {want}` would describe a \
                 check the source does not demonstrate",
                got.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    problems
}
