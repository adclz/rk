// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A file's AST and its syntax errors, as the compiler reads them.
//!
//! auto-lsp's `get_ast` accumulates the syntax errors tree-sitter finds, and
//! salsa marks every query calling one that accumulated, then every query
//! calling those: the mark is on the result, whether anyone reads the errors
//! or not. Salsa's fixpoint iteration refuses to settle a cycle whose head
//! carries it, and HIR's `ancestry` and call graph are such cycles, so a
//! syntax error anywhere in a file made them panic. Here the errors are part
//! of the result.

use auto_lsp::core::errors::{LexerError, ParseError};
use auto_lsp::default::db::{file::File, tracked::ParsedAst};
use auto_lsp::tree_sitter::Node;

use crate::WorkspaceDataBase;

/// A file's AST, and why parts of it are missing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub ast: ParsedAst,
    /// The syntax errors tree-sitter found, in the order of the text.
    pub syntax_errors: Vec<ParseError>,
    /// Why no AST could be built at all, when none could.
    pub failure: Option<ParseError>,
    /// The pragmas the grammar has no rule for, in the order of the text.
    /// The parser reads one as a token wherever it stands and the AST has
    /// no place for it, so the compiler reports it from here (E1510).
    pub unknown_pragmas: Vec<UnknownPragma>,
}

/// A pragma the grammar has no rule for: `{attribute 'hide'}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPragma {
    pub range: auto_lsp::tree_sitter::Range,
    /// The first word after the `{`, `attribute`; empty when there is none.
    pub name: String,
}

/// The AST of `file` and its syntax errors, in the result rather than
/// accumulated. A node auto-lsp's builder has no place for is still
/// accumulated, by the builder, on this query: the generated AST not
/// matching the grammar, a compiler bug the grammar no longer produces.
#[salsa::tracked(returns(ref))]
pub fn parse(db: &dyn WorkspaceDataBase, file: File) -> Parsed {
    let doc = file.document(db);
    if doc.is_empty() {
        return Parsed::default();
    }
    let mut syntax_errors = Vec::new();
    collect_syntax_errors(&doc.tree.root_node(), doc.as_bytes(), &mut syntax_errors);
    let unknown_pragmas = unknown_pragmas(&doc.tree.root_node(), doc.as_bytes());
    match (file.parsers(db).ast_parser)(db, doc) {
        Ok(nodes) => Parsed {
            ast: ParsedAst::new(nodes),
            syntax_errors,
            failure: None,
            unknown_pragmas,
        },
        Err(failure) => Parsed {
            ast: ParsedAst::default(),
            syntax_errors,
            failure: Some(failure),
            unknown_pragmas,
        },
    }
}

/// Every `pragma` node under `root`. One is an extra and can stand anywhere
/// in the tree, but it is a token that starts with `{`, so the node at each
/// `{` of the text is looked up instead of walking every node, which cost a
/// fifth of the parse.
fn unknown_pragmas(root: &Node, source: &[u8]) -> Vec<UnknownPragma> {
    let pragma = root.language().id_for_node_kind("pragma", true);
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(offset) = source[at..].iter().position(|byte| *byte == b'{') {
        let start = at + offset;
        at = start + 1;
        // A `{` in a string, a comment or a known pragma is no pragma's.
        let Some(node) = root
            .descendant_for_byte_range(start, at)
            .filter(|node| node.kind_id() == pragma && node.start_byte() == start)
        else {
            continue;
        };
        let text = node.utf8_text(source).unwrap_or_default();
        let name = text
            .trim_start_matches('{')
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        found.push(UnknownPragma {
            range: node.range(),
            name,
        });
        at = node.end_byte();
    }
    found
}

/// The error nodes under `node`, worded as auto-lsp words them: a node with
/// an error none of its children has is one.
fn collect_syntax_errors(node: &Node, source: &[u8], errors: &mut Vec<ParseError>) {
    if !node.has_error() {
        return;
    }
    let mut cursor = node.walk();
    if node.children(&mut cursor).any(|child| child.has_error()) {
        for child in node.children(&mut cursor) {
            collect_syntax_errors(&child, source, errors);
        }
    } else {
        errors.push(syntax_error(node, source).into());
    }
}

fn syntax_error(node: &Node, source: &[u8]) -> LexerError {
    if node.is_missing() {
        LexerError::Missing {
            range: node.range(),
            error: format!("missing '{}'", node.grammar_name()),
            grammar_name: node.grammar_name(),
        }
    } else {
        let children: Vec<String> = (0..node.child_count())
            .filter_map(|i| node.child(i as u32))
            .map(|child| child.utf8_text(source).unwrap_or_default().to_string())
            .collect();
        LexerError::Syntax {
            range: node.range(),
            error: format!("unexpected token(s): '{}'", children.join(" ")),
            affected: children.join(" "),
        }
    }
}
