// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A file's AST, its syntax errors and its comments, as the compiler reads
//! them.
//!
//! auto-lsp's `get_ast` accumulates the syntax errors tree-sitter finds, and
//! salsa marks every query calling one that accumulated, then every query
//! calling those: the mark is on the result, whether anyone reads the errors
//! or not. Salsa's fixpoint iteration refuses to settle a cycle whose head
//! carries it, and HIR's `ancestry` and call graph are such cycles, so a
//! syntax error anywhere in a file made them panic. Here the errors are part
//! of the result.

use auto_lsp::core::errors::{LexerError, ParseError, ParseErrorAccumulator};
use auto_lsp::default::db::{BaseDatabase, file::File, tracked::ParsedAst};
use auto_lsp::tree_sitter::{Node, Range};

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
    /// no place for it, so the compiler reports it from here (E1510). Of
    /// rk's own files only.
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
/// Those are read through [`unbuilt_nodes`].
#[salsa::tracked(returns(ref))]
pub fn parse(db: &dyn WorkspaceDataBase, file: File) -> Parsed {
    let doc = file.document(db);
    if doc.is_empty() {
        return Parsed::default();
    }
    let mut syntax_errors = Vec::new();
    collect_syntax_errors(&doc.tree.root_node(), doc.as_bytes(), &mut syntax_errors);
    // The pragmas are rk's, and so is the fault of writing one it has no
    // rule for. A file of another kind is its front-end's to report on.
    let unknown_pragmas = if crate::loader::is_rk_parser(file.parsers(db)) {
        unknown_pragmas(&doc.tree.root_node(), doc.as_bytes())
    } else {
        Vec::new()
    };
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

/// The nodes auto-lsp's builder had no place for in `file`'s AST, which it
/// accumulates on [`parse`] instead of returning them. Read through this
/// query, never from the accumulator: salsa cannot track a read of
/// accumulated values, so the reader runs again in every revision, and so
/// does every query that reads the reader. This one runs again too, but it
/// compares what it finds, and its readers run again only when that changed.
#[salsa::tracked(returns(ref))]
pub fn unbuilt_nodes(db: &dyn WorkspaceDataBase, file: File) -> Vec<ParseError> {
    parse::accumulated::<ParseErrorAccumulator>(db, file)
        .into_iter()
        .map(|error| error.0.clone())
        .collect()
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

/// A comment of a file: where it is, and which of the three it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Comment {
    pub range: Range,
    pub kind: CommentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    /// `// …`, to the end of the line.
    Line,
    /// `/* … */`.
    C,
    /// `(* … *)`.
    Pascal,
}

/// The comments of a file, in the order of the text: the editor's
/// documentation and links read them, and the linter. A comment nested in
/// another is part of its token. Only the nodes that hold a `//`, a `(*` or
/// a `/*` are visited, so a file costs a scan of its bytes and a descent to
/// each comment. A query visited every node of the tree, which was most of
/// what the editor's comment index cost.
#[salsa::tracked(returns(ref))]
pub fn comments(db: &dyn BaseDatabase, file: File) -> Vec<Comment> {
    let document = file.document(db);
    let source = document.as_bytes();
    // Where a comment may start, in order. One inside a string is a false
    // start, which only costs a descent.
    let starts: Vec<usize> = source
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| matches!(pair, [b'/', b'/'] | [b'(', b'*'] | [b'/', b'*']))
        .map(|(at, _)| at)
        .collect();
    let mut found = Vec::new();
    if starts.is_empty() {
        return found;
    }
    let holds_a_start = |node: Node| {
        let first = starts.partition_point(|&at| at < node.start_byte());
        starts.get(first).is_some_and(|&at| at < node.end_byte())
    };
    let mut cursor = document.tree.walk();
    loop {
        let node = cursor.node();
        let kind = match node.kind() {
            "line_comment" => Some(CommentKind::Line),
            "c_style_comment" => Some(CommentKind::C),
            "pascal_style_comment" => Some(CommentKind::Pascal),
            _ => None,
        };
        if let Some(kind) = kind {
            found.push(Comment {
                range: node.range(),
                kind,
            });
        } else if holds_a_start(node) && cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return found;
            }
        }
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
