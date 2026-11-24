// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The spaces between tokens, as the parser reads them.
//!
//! A space between two tokens only keeps them apart. Written with no space
//! where two tokens stay two, a program must parse to the same tree. A token
//! read by its context breaks that: in a CASE body, `x+1` was `x` and the
//! signed literal `+1`, where `x + 1` was a sum (E0001).

use auto_lsp::tree_sitter::{Node, Parser, Tree};

use crate::Finding;

pub fn check(source: &str) -> Result<(), Finding> {
    // Recovery reads a program with a syntax error, and may read it by
    // where its spaces are.
    if !formatter::accepts(source) {
        return Ok(());
    }
    let Some(spaced) = parse(source) else {
        return Ok(());
    };
    let tight = tight(source, spaced.root_node());
    let Some(again) = parse(&tight) else {
        return Err(Finding::new(
            "spacing",
            "written with no optional space, the program does not parse",
        ));
    };
    let (before, after) = (shape(spaced.root_node()), shape(again.root_node()));
    if before == after {
        return Ok(());
    }
    let (spaced_tokens, tight_tokens) = (
        tokens(source, spaced.root_node()),
        tokens(&tight, again.root_node()),
    );
    let at = (0..spaced_tokens.len().max(tight_tokens.len()))
        .find(|&i| spaced_tokens.get(i) != tight_tokens.get(i))
        .unwrap_or(0);
    let near = |tokens: &[(&'static str, String)]| -> String {
        let from = at.saturating_sub(3);
        tokens
            .iter()
            .skip(from)
            .take(6)
            .map(|(kind, text)| format!("{text} ({kind})"))
            .collect::<Vec<_>>()
            .join("  ")
    };
    Err(Finding::new(
        "spacing",
        format!(
            "written with no optional space, the program parses to another tree:\n  spaced: {}\n  tight:  {}",
            near(&spaced_tokens),
            near(&tight_tokens)
        ),
    ))
}

fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rk::LANGUAGE.into()).ok()?;
    parser.parse(source, None)
}

/// The program written again from its tokens, with no space between two
/// tokens that stay two, and a line break after a line comment.
fn tight(source: &str, root: Node<'_>) -> String {
    let mut out = String::new();
    let mut last: Option<(Node<'_>, &str)> = None;
    for leaf in leaves(root) {
        let text = &source[leaf.byte_range()];
        if text.is_empty() {
            continue;
        }
        if let Some((node, before)) = last {
            if before.starts_with("//") {
                out.push('\n');
            } else if glued(before, text) || signed(node, text) {
                out.push(' ');
            }
        }
        out.push_str(text);
        last = Some((leaf, text));
    }
    out
}

/// Whether `sign` is a sign of its own before the digits of `right`, which
/// written together are one signed literal: `- 1` is the sign applied to
/// `1`, `-1` the literal. A `+` or a `-` between two operands is glued: there
/// the digits must stay the right operand, whatever context lexes them.
fn signed(sign: Node<'_>, right: &str) -> bool {
    // `unary`, a supertype, is no node: the sign hangs from the operator.
    sign.parent()
        .is_some_and(|parent| parent.kind() == "unary_operator")
        && right.starts_with(|c: char| c.is_ascii_digit())
}

/// The operators and comment marks of two characters or more, which two
/// tokens written together could make.
const LONGER: [&str; 17] = [
    ":=", "=>", "<=", ">=", "<>", "**", "..", "+=", "-=", "*=", "/=", "(*", "*)", "/*", "*/", "//",
    "?=",
];

/// Whether `left` and `right`, written with nothing between them, would
/// read as one token or open a comment: two words, two quoted texts, or the
/// start of a longer operator.
fn glued(left: &str, right: &str) -> bool {
    let (Some(a), Some(b)) = (left.chars().last(), right.chars().next()) else {
        return false;
    };
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let quote = |c: char| c == '\'' || c == '"';
    if (word(a) && word(b)) || (quote(a) && quote(b)) {
        return true;
    }
    let pair: String = [a, b].iter().collect();
    LONGER.iter().any(|op| op.starts_with(&pair))
}

/// The tokens of a tree, in order: its leaves, comments included.
fn leaves(root: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.child_count() == 0 {
            out.push(node);
        }
        if cursor.goto_first_child() || cursor.goto_next_sibling() {
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return out;
            }
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Each token's kind and text, for a finding.
fn tokens(source: &str, root: Node<'_>) -> Vec<(&'static str, String)> {
    leaves(root)
        .into_iter()
        .filter(|leaf| !leaf.is_extra())
        .map(|leaf| (leaf.kind(), source[leaf.byte_range()].to_string()))
        .collect()
}

/// A tree as a comparable value, whatever its spaces: each node's depth,
/// kind and error state, in order. Comments are left out: where one hangs
/// is not what the program means.
fn shape(root: Node<'_>) -> Vec<(usize, u16, bool, bool)> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    let mut depth = 0;
    loop {
        let node = cursor.node();
        if !node.is_extra() {
            out.push((depth, node.kind_id(), node.is_missing(), node.is_error()));
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        if cursor.goto_next_sibling() {
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return out;
            }
            depth -= 1;
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No space is left where two tokens stay two. One is kept between two
    /// words, two quoted texts, the halves of an operator, and a sign of
    /// its own and its digits, and a line comment ends its line.
    #[test]
    fn a_program_is_written_tight() {
        let source = "PROGRAM P\nVAR\n    x : INT := 1; // one\n    s : STRING := 'a';\nEND_VAR\n    x := x + 1;\n    x := x - -1;\n    x := - 1;\n    IF x <= 2 THEN x := x ** 2; END_IF;\nEND_PROGRAM\n";
        let tree = parse(source).unwrap();
        assert_eq!(
            tight(source, tree.root_node()),
            "PROGRAM P VAR x:INT:=1;// one\ns:STRING:='a';END_VAR x:=x+1;x:=x--1;x:=- 1;IF x<=2 THEN x:=x**2;END_IF;END_PROGRAM"
        );
        assert_eq!(check(source), Ok(()));
    }

    /// A sum in a CASE body reads as a sum without its spaces: `x+1` was
    /// `x` and the signed literal `+1`.
    #[test]
    fn a_case_sum_keeps_its_tree() {
        let source = "FUNCTION F : INT\nVAR_INPUT k : INT; x : INT; END_VAR\n    CASE k OF\n        1: F := x + 1;\n        2: F := x - 16#10;\n    END_CASE;\nEND_FUNCTION\n";
        assert_eq!(check(source), Ok(()));
    }

    /// Every piece of ST the repository holds keeps its tree written tight:
    /// the seed corpus, which the fuzzers start from.
    #[test]
    fn the_repository_keeps_its_trees() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let seeds = std::env::temp_dir().join(format!("rk-spacing-{}", std::process::id()));
        crate::create_seed_corpus(&root, &seeds, 0).expect("the seed corpus");
        let mut findings = Vec::new();
        let mut checked = 0;
        for entry in std::fs::read_dir(&seeds).expect("the seeds").flatten() {
            let Ok(source) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            checked += 1;
            if let Err(finding) = check(&source) {
                findings.push(format!("{}: {finding}", entry.path().display()));
            }
        }
        let _ = std::fs::remove_dir_all(&seeds);
        assert!(checked > 1000, "only {checked} seeds");
        assert!(findings.is_empty(), "{}", findings.join("\n"));
    }
}
