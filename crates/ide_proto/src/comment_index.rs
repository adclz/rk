use std::sync::LazyLock;

use auto_lsp::{
    core::{document::Document, span::Span},
    default::db::{BaseDatabase, file::File},
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

#[tracing::instrument(skip_all, name = "query_comment_index")]
#[salsa::tracked(returns(ref))]
pub fn comment_index(db: &dyn BaseDatabase, file: File) -> CommentIndex {
    let mut map = FxHashMap::default();
    let mut query_cursor = tree_sitter::QueryCursor::new();
    let mut captures = query_cursor.captures(
        &COMMENT_QUERY,
        file.document(db).tree.root_node(),
        file.document(db).as_bytes(),
    );
    // Since the standard supports nested comments,
    // we need to carefully ignore them if they are nested
    let mut curr_range: Option<Span> = None;

    while let Some((capture, capture_index)) = captures.next() {
        let capture = capture.captures[*capture_index];
        let kind = match COMMENT_QUERY.capture_names()[capture.index as usize] {
            "line_comment" => CommentKind::Line,
            "c_comment" => CommentKind::C,
            "pascal_comment" => CommentKind::Pascal,
            _ => continue,
        };

        let node = capture.node;
        let range: Span = node.range().into();

        match curr_range {
            Some(curr) => {
                if range.start_byte >= curr.start_byte && range.end_byte <= curr.end_byte {
                    // Ignore nested comments
                    continue;
                } else {
                    // Update current range for new non-nested comment
                    curr_range = Some(range);
                }
            }
            None => {
                // Initialize the current range if it's not set
                curr_range = Some(range);
            }
        }

        let comment = Comment { range, kind };

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
        range: &Span,
    ) -> Option<&Comment> {
        let line = range.start_point.row;
        let column = range.start_point.column;

        // First: check lines above
        for row in (0..line).rev() {
            if let Some(comment) = self.map.get(&row) {
                // Check if the comment is actually above the line (not to the right)
                let text = document
                    .as_str()
                    .get(comment.range.start_byte..comment.range.end_byte)
                    .unwrap_or("");

                if match comment.kind {
                    CommentKind::C => text.starts_with("/*"),
                    CommentKind::Pascal => text.starts_with("(*"),
                    CommentKind::Line => text.starts_with("//"),
                } {
                    return Some(comment);
                }
            }
            if let Some(line_content) = document.texter.get_row(row)
                && !line_content.is_empty()
            {
                // Still no comment, but there's a non-empty line
                break;
            }
        }

        // Second: check for comments on the same line
        let same_line_comments = self.map.get(&line);
        let mut best_right: Option<&Comment> = None;

        if let Some(comment) = same_line_comments
            && comment.range.start_point.column >= column
        {
            match &best_right {
                Some(existing) => {
                    if comment.range.start_point.column < existing.range.start_point.column {
                        best_right = Some(comment);
                    }
                }
                None => best_right = Some(comment),
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
    pub range: Span,
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
                let content = text.strip_prefix("/*").unwrap_or(text);
                let content = content.strip_suffix("*/").unwrap_or(content);
                Self::dedent_block(content)
            }
            CommentKind::Pascal => {
                let content = text.strip_prefix("(*").unwrap_or(text);
                let content = content.strip_suffix("*)").unwrap_or(content);
                Self::dedent_block(content)
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
}
