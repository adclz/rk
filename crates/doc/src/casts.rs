//! The cast tables in the README, derived so they cannot drift from the
//! compiler: `ElementarySpec::implicit_cast` is what an assignment widens on
//! its own, and the `X_TO_Y` functions of `Std.Convert` are what a program
//! can call. The compiler's own `explicit_cast` — the table behind E0301's
//! "consider explicitly casting with" hint — is checked against those
//! functions, so the hint can never name one that does not exist.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use hir::hir_def::expressions::spec::ElementarySpec as E;

/// Every elementary type a program can declare, in the standard's order.
/// The two edge specs are declaration qualifiers, not types.
const TYPES: [E; 25] = [
    E::Bool,
    E::Byte,
    E::Word,
    E::DWord,
    E::LWord,
    E::SInt,
    E::Int,
    E::DInt,
    E::LInt,
    E::USInt,
    E::UInt,
    E::UDInt,
    E::ULInt,
    E::Real,
    E::LReal,
    E::Char,
    E::String,
    E::Time,
    E::LTime,
    E::Date,
    E::LDate,
    E::Tod,
    E::LTod,
    E::DateAndTime,
    E::LDateTime,
];

const BEGIN: &str = "<!-- casts:begin -->";
const END: &str = "<!-- casts:end -->";

/// Rewrite what lies between the markers, markers kept. True when the file
/// changed, which in CI means the committed copy was stale.
pub fn refresh_readme(path: &Path, convert_st: &str) -> bool {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let (start, end) = marked(&text).or_else(|| hand_kept(&text)).unwrap_or_else(|| {
        panic!(
            "{}: the cast tables are gone. Put `{BEGIN}` and `{END}` on two lines where they belong, and run again",
            path.display()
        )
    });
    let new = format!(
        "{}{BEGIN}\n\n{}\n\n{END}{}",
        &text[..start],
        tables(convert_st),
        &text[end..]
    );
    if new == text {
        return false;
    }
    std::fs::write(path, new).unwrap();
    true
}

/// The byte range the markers enclose, markers included.
fn marked(text: &str) -> Option<(usize, usize)> {
    let start = text.find(BEGIN)?;
    let end = start + text[start..].find(END)? + END.len();
    Some((start, end))
}

/// The same block with its markers lost, which an undo or a paste of an older
/// copy does without anyone noticing. It is recognised by its two summaries,
/// so the README heals itself instead of stopping the build: from the
/// `<details>` that opens the Implicit table to the `</details>` that closes
/// the Explicit one.
fn hand_kept(text: &str) -> Option<(usize, usize)> {
    const OPEN: &str = "<details>";
    const CLOSE: &str = "</details>";
    let implicit = text.find("Implicit</strong>")?;
    let start = text[..implicit].rfind(OPEN)?;
    if text[start..implicit].contains(CLOSE) {
        return None;
    }
    let explicit = implicit + text[implicit..].find("Explicit</strong>")?;
    let end = explicit + text[explicit..].find(CLOSE)? + CLOSE.len();
    Some((start, end))
}

/// Both tables as Markdown, one row per source type.
pub fn tables(convert_st: &str) -> String {
    let functions = convert_functions(convert_st);
    check_hints_exist(&functions);

    // Each table folds under a `<details>`, as GitHub draws one. The blank
    // line after `</summary>` is load-bearing: it ends the HTML block, and
    // only then does Markdown parse the table that follows.
    let mut out = String::new();
    out.push_str("<details>\n<summary><strong>Implicit</strong>, what an assignment or a call widens on its own.</summary>\n\n");
    out.push_str("| from | to |\n|---|---|\n");
    for from in TYPES {
        let to: Vec<&str> = TYPES
            .iter()
            .filter(|to| **to != from && to.implicit_cast(from).is_some())
            .map(|to| to.type_name())
            .collect();
        out.push_str(&row(from.type_name(), &to));
    }

    out.push_str("\n</details>\n\n<details>\n<summary><strong>Explicit</strong>, the <code>Std.Convert</code> functions, each named <code>FROM_TO_TO</code>.</summary>\n\n");
    out.push_str("| from | to |\n|---|---|\n");
    let mut by_from: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (from, to) in &functions {
        by_from.entry(index(from)).or_default().push(index(to));
    }
    for (i, from) in TYPES.iter().enumerate() {
        let mut to = by_from.remove(&i).unwrap_or_default();
        to.sort_unstable();
        let to: Vec<&str> = to.into_iter().map(|j| TYPES[j].type_name()).collect();
        out.push_str(&row(from.type_name(), &to));
    }
    out.push_str("\n</details>");
    out
}

fn row(from: &str, to: &[&str]) -> String {
    let to = if to.is_empty() {
        "—".to_string()
    } else {
        to.iter()
            .map(|t| format!("`{t}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("| `{from}` | {to} |\n")
}

fn index(name: &str) -> usize {
    TYPES
        .iter()
        .position(|t| t.type_name() == name)
        .unwrap_or_else(|| panic!("Std.Convert names a type the compiler does not have: `{name}`"))
}

/// `(FROM, TO)` for every public `FUNCTION FROM_TO_TO` in the file. The
/// helpers are `PRIVATE`, `TRUNC` has no `_TO_`, and the tests are lowercase.
fn convert_functions(convert_st: &str) -> BTreeSet<(String, String)> {
    convert_st
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("FUNCTION ")?;
            let name = rest.split([':', ' ', '\t']).next()?;
            if !name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') {
                return None;
            }
            let (from, to) = name.split_once("_TO_")?;
            (!from.is_empty() && !to.is_empty()).then(|| (from.to_string(), to.to_string()))
        })
        .collect()
}

/// Every cast the compiler would suggest must be a function that exists.
fn check_hints_exist(functions: &BTreeSet<(String, String)>) {
    let missing: Vec<String> = TYPES
        .iter()
        .flat_map(|from| TYPES.iter().map(move |to| (*from, *to)))
        .filter(|(from, to)| from != to && to.explicit_cast(*from))
        .map(|(from, to)| (from.type_name().to_string(), to.type_name().to_string()))
        .filter(|pair| !functions.contains(pair))
        .map(|(from, to)| format!("{from}_TO_{to}"))
        .collect();
    assert!(
        missing.is_empty(),
        "the compiler suggests casts that Std.Convert does not define: {}",
        missing.join(", ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONVERT: &str = include_str!("../../../stdlib/Convert.st");

    #[test]
    fn tables_follow_the_standard_and_the_stdlib() {
        let t = tables(CONVERT);
        // The README's own examples: INT widens to REAL, DINT does not.
        assert!(
            t.contains("| `INT` | `DINT`, `LINT`, `REAL`, `LREAL` |"),
            "{t}"
        );
        assert!(t.contains("| `DINT` | `LINT`, `LREAL` |"), "{t}");
        // A STRING never widens, and everything can become one explicitly.
        assert!(t.contains("| `STRING` | — |"), "{t}");
        assert!(t.contains("| `REAL` | `DWORD`, `SINT`, `INT`"), "{t}");
        // Two folded blocks, each with the blank line that lets its table parse.
        assert_eq!(t.matches("</summary>\n\n| from | to |").count(), 2, "{t}");
        assert!(t.ends_with("|\n\n</details>"), "{t}");
    }

    #[test]
    fn a_block_that_lost_its_markers_is_wrapped_again() {
        let dir = std::env::temp_dir().join("rk-doc-casts-heal-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("README.md");
        // As an editor leaves it: a blank line after `<details>`, a typo in
        // the second summary, and no marker anywhere.
        std::fs::write(
            &path,
            "### Strict casts\n\n<details>\n\n<summary><strong>Implicit</strong>, x.</summary>\n\n| from | to |\n|---|---|\n| `A` | `B` |\n\n</details>\n\n<details>\n\n<summary><strong>,Explicit</strong>, y.</summary>\n\n| from | to |\n|---|---|\n| `STALE` | `ROW` |\n\n</details>\n\nAfter.\n",
        )
        .unwrap();
        assert!(refresh_readme(&path, CONVERT));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("### Strict casts\n\n<!-- casts:begin -->\n\n<details>\n<summary>"),
            "{text}"
        );
        assert!(
            text.ends_with("</details>\n\n<!-- casts:end -->\n\nAfter.\n"),
            "{text}"
        );
        assert!(
            !text.contains("STALE") && !text.contains(",Explicit"),
            "{text}"
        );
        assert_eq!(text.matches("<details>").count(), 2, "{text}");
        assert!(!refresh_readme(&path, CONVERT), "healed once, then stable");
    }

    #[test]
    fn a_stale_readme_is_rewritten_once() {
        let dir = std::env::temp_dir().join("rk-doc-casts-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("README.md");
        std::fs::write(
            &path,
            "before\n\n<!-- casts:begin -->\nold\n<!-- casts:end -->\n\nafter\n",
        )
        .unwrap();
        assert!(refresh_readme(&path, CONVERT));
        assert!(
            !refresh_readme(&path, CONVERT),
            "a second run must change nothing"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(
            "before\n\n<!-- casts:begin -->\n\n<details>\n<summary><strong>Implicit</strong>"
        ));
        assert!(text.ends_with("<!-- casts:end -->\n\nafter\n"));
    }
}
