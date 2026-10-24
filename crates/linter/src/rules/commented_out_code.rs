// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A comment that holds statements. Its text is parsed as the body of a
//! FUNCTION: a comment that parses, with a `;` in it and a statement among
//! what it parses to, is code. Line comments on consecutive lines are one
//! comment, so a block commented out line by line is reported once.

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "commented-out-code";

/// L0318: statements left in a comment.
struct CommentedOutCode;

impl ErrorCode for CommentedOutCode {
    fn code(&self) -> &'static str {
        "L0318"
    }
}

/// The statement kinds of the grammar: what makes a parsed text code.
const STATEMENTS: &[&str] = &[
    "assign",
    "func_call",
    "if_stmt",
    "case_stmt",
    "for_stmt",
    "while_stmt",
    "repeat_stmt",
    "raise_stmt",
    "try_stmt",
];

/// One comment, or a run of line comments on consecutive lines.
struct Comment {
    range: tree_sitter::Range,
    text: String,
}

pub fn check(db: &dyn WorkspaceDataBase, file: File, diagnostics: &mut Vec<IdeDiagnostic>) {
    // The tree is rk's own only in rk's own files.
    if !db::loader::is_rk_parser(file.parsers(db)) {
        return;
    }
    // One parser for the file's comments.
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_rk::LANGUAGE.into())
        .is_err()
    {
        return;
    }
    for comment in comments(db, file) {
        if !is_code(&mut parser, &comment.text) {
            continue;
        }
        let mut d = diag()
            .message("this comment contains code".to_string())
            .desc(&CommentedOutCode)
            .range(hir::denormalize(db, file, &comment.range).unwrap_or_default())
            .severity(DiagnosticSeverity::HINT)
            .call();
        d.with_note("code in a comment is easily put back by mistake".to_string());
        d.with_help("delete the commented-out code".to_string());
        diagnostics.push(d);
    }
}

fn comments(db: &dyn WorkspaceDataBase, file: File) -> Vec<Comment> {
    let source = file.document(db).as_bytes();
    let mut out: Vec<Comment> = Vec::new();
    // Whether the last comment was a line comment, which the next one on
    // the following line continues.
    let mut last_was_line = false;
    for comment in db::syntax::comments(db, file) {
        let line = comment.kind == db::syntax::CommentKind::Line;
        let range = comment.range;
        let raw = String::from_utf8_lossy(&source[range.start_byte..range.end_byte]);
        let text = if line {
            raw.strip_prefix("//").unwrap_or(&raw).to_string()
        } else {
            raw.get(2..raw.len().saturating_sub(2))
                .unwrap_or_default()
                .to_string()
        };
        match out.last_mut() {
            Some(previous)
                if line
                    && last_was_line
                    && range.start_point.row == previous.range.end_point.row + 1 =>
            {
                previous.range.end_byte = range.end_byte;
                previous.range.end_point = range.end_point;
                previous.text.push('\n');
                previous.text.push_str(&text);
            }
            _ => out.push(Comment { range, text }),
        }
        last_was_line = line;
    }
    out
}

/// Whether `text` parses as statements, one of them at least.
fn is_code(parser: &mut tree_sitter::Parser, text: &str) -> bool {
    // A statement has a `;`, and a backtick is no token of the language: a
    // comment with a markdown fence is documentation, which the parser
    // would only fail on, slowly, recovering from every line of prose.
    if !text.contains(';') || text.contains('`') {
        return false;
    }
    let source = format!("FUNCTION rk_comment\n{text}\nEND_FUNCTION\n");
    let Some(tree) = parser.parse(&source, None) else {
        return false;
    };
    let root = tree.root_node();
    if root.has_error() {
        return false;
    }
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if STATEMENTS.contains(&node.kind()) {
            return true;
        }
        stack.extend(node.children(&mut cursor));
    }
    false
}
