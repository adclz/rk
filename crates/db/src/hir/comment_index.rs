use std::sync::LazyLock;

use auto_lsp::{
    core::document::Document,
    default::db::{file::File, BaseDatabase},
    tree_sitter::{self, StreamingIterator},
};
use rustc_hash::FxHashMap;

static COMMENT_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(
        &tree_sitter_rk::LANGUAGE.into(),
        "
(line_comment) @line_comment
(c_style_comment) @c_comment
(pascal_style_comment) @pascal_comment
",
    )
    .unwrap()
});

#[salsa::tracked(returns(ref), no_eq)]
pub fn comment_index(db: &dyn BaseDatabase, file: File) -> CommentIndex {
    let mut map = FxHashMap::default();

    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(
        &COMMENT_QUERY,
        file.document(db).tree.root_node(),
        file.document(db).as_bytes(),
    );

    // Since the standard supports nested comments,
    // we need to carefully ignores them if they are nested
    let mut curr_range: Option<tree_sitter::Range> = None;

    while let Some((capture, capture_index)) = captures.next() {
        let capture = capture.captures[*capture_index];
        let kind = match COMMENT_QUERY.capture_names()[capture.index as usize] {
            "line_comment" => CommentKind::Line,
            "c_comment" => CommentKind::C,
            "pascal_comment" => CommentKind::Pascal,
            _ => continue,
        };

        let node = capture.node;
        let range = node.range();

        match curr_range {
            Some(curr) => {
                if range.start_byte >= curr.start_byte && range.end_byte <= curr.end_byte {
                    // Ignore nested comments
                    continue;
                } else {
                    // Update current range for new non-nested comment
                    curr_range = Some(range.clone());
                }
            }
            None => {
                // Initialize the current range if it's not set
                curr_range = Some(range.clone());
            }
        }

        let comment = Comment {
            range: range.clone(),
            kind,
        };

        map.insert(range.end_point.row, comment);
    }
    CommentIndex { map }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentIndex {
    pub map: FxHashMap<usize, Comment>,
}

impl CommentIndex {
    pub fn find_nearby_comment(
        &self,
        document: &Document,
        range: &tree_sitter::Range,
    ) -> Option<&Comment> {
        let line = range.start_point.row;
        let column = range.start_point.column;

        // First: check lines above
        for row in (0..line).rev() {
            if let Some(comment) = self.map.get(&row) {
                return Some(comment);
            }
            if let Some(line_content) = document.texter.get_row(row) {
                if !line_content.is_empty() {
                    // Still no comment, but we found a non-empty line
                    break;
                }
            }
        }

        // Second: check for comments on the same line
        let same_line_comments = self.map.get(&line);
        let mut best_right: Option<&Comment> = None;

        if let Some(comment) = same_line_comments {
            if comment.range.start_point.column >= column {
                match &best_right {
                    Some(existing) => {
                        if comment.range.start_point.column < existing.range.start_point.column {
                            best_right = Some(comment);
                        }
                    }
                    None => best_right = Some(comment),
                }
            }
        }

        if best_right.is_some() {
            return best_right;
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Comment {
    pub range: tree_sitter::Range,
    pub kind: CommentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    Line,
    C,
    Pascal,
}

impl Comment {
    pub fn to_string(&self, document: &Document) -> String {
        let text = document
            .as_str()
            .get(self.range.start_byte..self.range.end_byte)
            .unwrap_or("");

        match self.kind {
            CommentKind::Line => text.replace("//", "").trim_start().to_string(),
            CommentKind::C => {
                let content = text.strip_prefix("/*").unwrap_or(text).trim_start();
                content.strip_suffix("*/").unwrap_or(content).trim_end().to_string()
            }
            CommentKind::Pascal => {
                let content = text.strip_prefix("(*").unwrap_or(text).trim_start();
                content.strip_suffix("*)").unwrap_or(content).trim_end().to_string()
            }
        }
    }
}