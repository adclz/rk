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

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types, tree_sitter};

    use crate::RootDatabase;

    use super::*;

    #[test]
    fn single_line_comment() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
// This is a single line comment
FUNCTION test


END_FUNCTION
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let index = comment_index(&db, file);

        let file = db.get_file(&url).unwrap();
        let document = file.document(&db);
        let range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 35,
            start_point: tree_sitter::Point { row: 2, column: 0 },
            end_point: tree_sitter::Point { row: 2, column: 0 },
        };
        let comment = index.find_nearby_comment(&document, &range).unwrap();

        assert_eq!(
            comment.to_string(&document),
            "This is a single line comment"
        );
    }

    #[test]
    fn comment_to_the_right_on_same_line() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///right.st").unwrap();
        let source = r#"
FUNCTION test //right side comment
    VAR
    END_VAR
END_FUNCTION
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let index = comment_index(&db, file);
        let file = db.get_file(&url).unwrap();
        let document = file.document(&db);

        let range = tree_sitter::Range {
            start_byte: 10,
            end_byte: 20,
            start_point: tree_sitter::Point { row: 1, column: 9 },
            end_point: tree_sitter::Point { row: 1, column: 9 },
        };

        let comment = index.find_nearby_comment(&document, &range).unwrap();
        assert_eq!(comment.to_string(&document), "right side comment");
    }

    #[test]
    fn blank_lines() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///blank.st").unwrap();
        let source = r#"
// Separated by blank lines



FUNCTION test


END_FUNCTION
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let index = comment_index(&db, file);
        let file = db.get_file(&url).unwrap();
        let document = file.document(&db);

        let range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 0,
            start_point: tree_sitter::Point { row: 5, column: 0 },
            end_point: tree_sitter::Point { row: 5, column: 0 },
        };
        let comment = index.find_nearby_comment(&document, &range).unwrap();
        assert_eq!(comment.to_string(&document), "Separated by blank lines");
    }

    #[test]
    fn c_style_comment() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///c_style.st").unwrap();
        let source = r#"
/* This is a C-style comment */
FUNCTION test
    
END_FUNCTION
"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let index = comment_index(&db, file);
        let file = db.get_file(&url).unwrap();
        let document = file.document(&db);

        let range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 0,
            start_point: tree_sitter::Point { row: 2, column: 0 },
            end_point: tree_sitter::Point { row: 2, column: 0 },
        };
        let comment = index.find_nearby_comment(&document, &range).unwrap();

        assert_eq!(comment.to_string(&document), "This is a C-style comment");
    }

    #[test]
    fn multiline_pascal_style_comment() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///pascal_style.st").unwrap();
        let source = r#"
(* This is a 
    multiline 
    Pascal-style comment 
*)
FUNCTION test
END_FUNCTION
"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let index = comment_index(&db, file);
        let file = db.get_file(&url).unwrap();
        let document = file.document(&db);

        let range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 60,
            start_point: tree_sitter::Point { row: 5, column: 0 },
            end_point: tree_sitter::Point { row: 5, column: 0 },
        };
        let comment = index.find_nearby_comment(&document, &range).unwrap();
        assert_eq!(
            comment.to_string(&document),
            "This is a \n    multiline \n    Pascal-style comment"
        );
    }

    #[test]
    fn ignore_nested_comments() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///nested_comments.st").unwrap();
        let source = r#"
(* 
  NOT NESTED
  (* NESTED *)
*) 
FUNCTION test
END_FUNCTION
"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let index = comment_index(&db, file);

        assert!(index.map.len() == 1, "Expected one comment in the index");

        let file = db.get_file(&url).unwrap();
        let document = file.document(&db);

        let range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 20,
            start_point: tree_sitter::Point { row: 5, column: 0 },
            end_point: tree_sitter::Point { row: 5, column: 0 },
        };
        let comment = index.find_nearby_comment(&document, &range).unwrap();
        assert_eq!(comment.to_string(&document), "NOT NESTED\n  (* NESTED *)");
    }
}
