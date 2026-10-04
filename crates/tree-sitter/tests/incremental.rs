//! An edit reparsed with the tree from before it gives the tree a fresh parse
//! gives, for every character of these sources deleted and typed back, and
//! for comment and pragma delimiters typed anywhere. The editor only ever
//! reparses this way.
//!
//! Comments and unknown pragmas were rules made of one token per character,
//! and those tokens matched whitespace: after an edit, a reused token could
//! take the whitespace before it, and the tree no longer matched the text.
//! As with the fuzzer's oracle, an edit whose result does not parse is not
//! compared: error recovery may depend on what the parser saw before.

use tree_sitter::{InputEdit, Node, Parser, Point};

const SOURCES: &[&str] = &[
    r#"FUNCTION f : INT
VAR x : INT; s : STRING; a : INT; b : INT; END_VAR
    x := 1; // a // b // c
    // (* not a block
    // *) nor this
    //* nor this
    x := 6 / 2; // 6 / 2
    s := '(* in a string *)'; (* a // inside *)
    x := a / (*c*) b;
    f := x; /* a (* b */
END_FUNCTION
"#,
    r#"(* a /* b *)
(* a (* b (* c *) b *) a *)
(**) (***) (* x *y* (z) *) /**/ /*/ */
FUNCTION_BLOCK Fb
VAR
    {attribute 'hide'}
    a : INT; // one
    b : INT {attribute 'a}b' (* c *)};
    c : INT (* c *);
    {attribute 'see http://x'}
    d : INT;
END_VAR
    {pack_mode := '1'}
    a := 1;
END_FUNCTION_BLOCK

{test}
FUNCTION t
END_FUNCTION
"#,
    // A night of the fuzzer found two edits of this one that the old
    // comment rules reparsed wrong.
    r#"
FUNCTION f : INT
VAR
    x : INT; // one
    y : INT; (* two *)
END_VAR // after the section
    x := 1; // one
    y := 2; (* two *)
    IF x = 1 THEN // why
        x := 3; // three
        // alone
    ELSE // other
        x := 4;
    END_IF; // closed
    // alone again
    f := x; /* last */
END_FUNCTION // done
"#,
];

fn parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rk::LANGUAGE.into())
        .expect("the grammar loads");
    parser
}

/// Every node's kind, extent and error state.
fn shape(root: Node<'_>) -> Vec<(u16, usize, usize, bool, bool)> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        out.push((
            node.kind_id(),
            node.start_byte(),
            node.end_byte(),
            node.is_missing(),
            node.is_error(),
        ));
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return out;
            }
        }
    }
}

fn point(text: &str, byte: usize) -> Point {
    let before = &text.as_bytes()[..byte];
    let row = before.iter().filter(|b| **b == b'\n').count();
    let line_start = before
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |i| i + 1);
    Point {
        row,
        column: byte - line_start,
    }
}

/// `old[at..at + removed]` replaced by `inserted`: whether the reparse and a
/// fresh parse disagree on a text that parses.
fn disagrees(parser: &mut Parser, old: &str, at: usize, removed: usize, inserted: &str) -> bool {
    let mut tree = parser.parse(old, None).expect("a tree");
    let new = format!("{}{inserted}{}", &old[..at], &old[at + removed..]);
    tree.edit(&InputEdit {
        start_byte: at,
        old_end_byte: at + removed,
        new_end_byte: at + inserted.len(),
        start_position: point(old, at),
        old_end_position: point(old, at + removed),
        new_end_position: point(&new, at + inserted.len()),
    });
    let reparsed = parser.parse(&new, Some(&tree)).expect("a tree");
    let fresh = parser.parse(&new, None).expect("a tree");
    !fresh.root_node().has_error() && shape(reparsed.root_node()) != shape(fresh.root_node())
}

#[test]
fn an_edit_reparses_to_the_fresh_tree() {
    let mut parser = parser();
    let mut wrong = Vec::new();
    for source in SOURCES {
        assert!(
            !parser.parse(source, None).unwrap().root_node().has_error(),
            "a source the test starts from parses"
        );
        for (at, ch) in source.char_indices() {
            let len = ch.len_utf8();
            let without = format!("{}{}", &source[..at], &source[at + len..]);
            if disagrees(&mut parser, source, at, len, "") {
                wrong.push(format!("delete {ch:?} at {at}"));
            }
            if disagrees(&mut parser, &without, at, 0, &ch.to_string()) {
                wrong.push(format!("type {ch:?} back at {at}"));
            }
            for delimiter in ["(*", "*)", "/*", "*/", "//", "{", "}"] {
                if disagrees(&mut parser, source, at, 0, delimiter) {
                    wrong.push(format!("type {delimiter:?} at {at}"));
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} edit(s) reparse to another tree than a fresh parse:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
