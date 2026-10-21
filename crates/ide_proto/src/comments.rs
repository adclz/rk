// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The comments the editor reads, from [`db::syntax::comments`]: the
//! documentation of a declaration, and whether a place is in a comment.

use auto_lsp::{
    core::document::Document,
    default::db::{BaseDatabase, file::File},
    tree_sitter,
};
use db::syntax::{Comment, CommentKind, comments};

/// The last of `comments` that ends on `row`. They are in the order of the
/// text, so their last rows are too.
fn ending_on(comments: &[Comment], row: usize) -> Option<&Comment> {
    let after = comments.partition_point(|comment| comment.range.end_point.row <= row);
    comments[..after]
        .last()
        .filter(|comment| comment.range.end_point.row == row)
}

/// The documentation of the node at `range`: a block comment on the lines
/// right above it, or else one after it on its line.
pub fn find_nearby_comment<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    range: &tree_sitter::Range,
) -> Option<&'db Comment> {
    let comments = comments(db, file);
    let document = file.document(db);
    let line = range.start_point.row;
    let column = range.start_point.column;

    // First: check lines above. Only block comments count as documentation;
    // `//` line comments are intentionally ignored (see `is_doc`).
    for row in (0..line).rev() {
        if let Some(comment) = ending_on(comments, row)
            && is_doc(comment)
        {
            return Some(comment);
        }
        if let Some(line_content) = document.texter.get_row(row)
            && !line_content.is_empty()
        {
            // Non-empty line that isn't a doc comment: stop searching upward.
            break;
        }
    }

    // Second: a doc comment trailing on the same line.
    ending_on(comments, line)
        .filter(|comment| is_doc(comment) && comment.range.start_point.column >= column)
}

/// Whether `offset` is inside one of the file's comments, its delimiters
/// included.
pub fn in_comment(db: &dyn BaseDatabase, file: File, offset: usize) -> bool {
    let comments = comments(db, file);
    let first = comments.partition_point(|comment| comment.range.end_byte < offset);
    comments
        .get(first)
        .is_some_and(|comment| comment.range.start_byte <= offset)
}

/// Whether a comment is documentation (shown in hover and used for
/// `[TypeName]` doc references). Only block comments `(* *)` / `/* */`
/// qualify; `//` line comments are intentionally excluded.
pub fn is_doc(comment: &Comment) -> bool {
    matches!(comment.kind, CommentKind::C | CommentKind::Pascal)
}

/// The text of a comment without its delimiters, a block comment dedented.
pub fn doc_text(comment: &Comment, document: &Document) -> String {
    let text = document
        .as_str()
        .get(comment.range.start_byte..comment.range.end_byte)
        .unwrap_or("");

    match comment.kind {
        CommentKind::Line => text.replace("//", "").trim_start().to_string(),
        CommentKind::C => {
            let content = text.strip_prefix("/*").unwrap_or(text);
            let content = content.strip_suffix("*/").unwrap_or(content);
            dedent_block(content)
        }
        CommentKind::Pascal => {
            let content = text.strip_prefix("(*").unwrap_or(text);
            let content = content.strip_suffix("*)").unwrap_or(content);
            dedent_block(content)
        }
    }
}

/// Dedent a multiline block comment by stripping the common leading whitespace
/// from all non-empty lines, then trimming leading/trailing blank lines.
fn dedent_block(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    // Find minimum indentation across non-empty lines
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    // Strip common indentation and trailing whitespace from each line
    let result: Vec<&str> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else if line.len() >= min_indent {
                line[min_indent..].trim_end()
            } else {
                line.trim()
            }
        })
        .collect();

    // Trim leading and trailing blank lines
    let start = result.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let end = result
        .iter()
        .rposition(|l| !l.is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);

    result[start..end].join("\n")
}
