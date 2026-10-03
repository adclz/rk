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
    match (file.parsers(db).ast_parser)(db, doc) {
        Ok(nodes) => Parsed {
            ast: ParsedAst::new(nodes),
            syntax_errors,
            failure: None,
        },
        Err(failure) => Parsed {
            ast: ParsedAst::default(),
            syntax_errors,
            failure: Some(failure),
        },
    }
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
