// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::path::Path;

use auto_lsp::{
    core::document::Document,
    default::db::{BaseDatabase, file::File},
    lsp_types,
    tree_sitter::{self, StreamingIterator},
};
use db::RootDatabase;
use db::syntax::{Comment, CommentKind, comments};
use ide_proto::comments::{doc_text, find_nearby_comment};

#[test]
fn line_comment_above_is_ignored() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
// This is a single line comment
FUNCTION test


END_FUNCTION
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let range = tree_sitter::Range {
        start_byte: 0,
        end_byte: 35,
        start_point: tree_sitter::Point { row: 2, column: 0 },
        end_point: tree_sitter::Point { row: 2, column: 0 },
    };
    // `//` line comments are not documentation (see `Comment::is_doc`).
    assert!(find_nearby_comment(&db, file, &range).is_none());
}

#[test]
fn line_comment_to_the_right_is_ignored() {
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
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();

    let range = tree_sitter::Range {
        start_byte: 10,
        end_byte: 20,
        start_point: tree_sitter::Point { row: 1, column: 9 },
        end_point: tree_sitter::Point { row: 1, column: 9 },
    };

    // A trailing `//` comment is not documentation.
    assert!(find_nearby_comment(&db, file, &range).is_none());
}

#[test]
fn line_comment_across_blank_lines_is_ignored() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///blank.st").unwrap();
    let source = r#"
// Separated by blank lines



FUNCTION test


END_FUNCTION
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();

    let range = tree_sitter::Range {
        start_byte: 0,
        end_byte: 0,
        start_point: tree_sitter::Point { row: 5, column: 0 },
        end_point: tree_sitter::Point { row: 5, column: 0 },
    };
    // A `//` comment (even adjacent across blank lines) is not documentation.
    assert!(find_nearby_comment(&db, file, &range).is_none());
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
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let document = file.document(&db);

    let range = tree_sitter::Range {
        start_byte: 0,
        end_byte: 0,
        start_point: tree_sitter::Point { row: 2, column: 0 },
        end_point: tree_sitter::Point { row: 2, column: 0 },
    };
    let comment = find_nearby_comment(&db, file, &range).unwrap();

    assert_eq!(doc_text(comment, document), "This is a C-style comment");
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
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let document = file.document(&db);

    let range = tree_sitter::Range {
        start_byte: 0,
        end_byte: 60,
        start_point: tree_sitter::Point { row: 5, column: 0 },
        end_point: tree_sitter::Point { row: 5, column: 0 },
    };
    let comment = find_nearby_comment(&db, file, &range).unwrap();
    assert_eq!(
        doc_text(comment, document),
        "This is a\n   multiline\n   Pascal-style comment"
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
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    assert!(
        comments(&db, file).len() == 1,
        "Expected one comment in the index"
    );

    let file = db.get_file(&url).unwrap();
    let document = file.document(&db);

    let range = tree_sitter::Range {
        start_byte: 0,
        end_byte: 20,
        start_point: tree_sitter::Point { row: 5, column: 0 },
        end_point: tree_sitter::Point { row: 5, column: 0 },
    };
    let comment = find_nearby_comment(&db, file, &range).unwrap();
    assert_eq!(doc_text(comment, document), "NOT NESTED\n(* NESTED *)");
}

#[test]
fn avoid_top_right_comment() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///right.st").unwrap();
    let source = r#"
FUNCTION test 
    VAR
        x : INT; //right side comment
        y : INT;
    END_VAR
END_FUNCTION
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    // Comment should only be picked *once* for x
    // y should not pick it up because the comment is to the right of x
    assert!(
        comments(&db, file).len() == 1,
        "Expected one comment in the index"
    );
}

/// The comments a query over the whole tree finds, as the editor's index
/// was built before it searched only where a comment may start: the
/// reference the search agrees with.
fn found_by_a_query(document: &Document) -> Vec<Comment> {
    let query = tree_sitter::Query::new(
        &tree_sitter_rk::LANGUAGE.into(),
        "(line_comment) @line (c_style_comment) @c (pascal_style_comment) @pascal",
    )
    .unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut captures = cursor.captures(&query, document.tree.root_node(), document.as_bytes());
    let mut found: Vec<Comment> = Vec::new();
    while let Some((capture, index)) = captures.next() {
        let capture = capture.captures[*index];
        let kind = match query.capture_names()[capture.index as usize] {
            "line" => CommentKind::Line,
            "c" => CommentKind::C,
            _ => CommentKind::Pascal,
        };
        found.push(Comment {
            range: capture.node.range(),
            kind,
        });
    }
    found
}

/// The search finds every comment a query over the whole tree finds, in
/// the standard library and where a search that only goes where a comment
/// may start could be misled: a nested comment, comments ending on one row,
/// comment marks in a string, comments in an expression and in a syntax
/// error.
#[test]
fn the_search_finds_what_a_query_over_the_tree_finds() {
    let stdlib = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib");
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&stdlib)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "st"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            (path.display().to_string(), text)
        })
        .collect();
    sources.sort();
    let snippets = [
        "(* outer (* inner *) outer *)\nFUNCTION f : INT\nEND_FUNCTION\n",
        "FUNCTION f : INT\nVAR x : INT; END_VAR\n    x := 1; (* a *) (* b *) // c\nEND_FUNCTION\n",
        "FUNCTION f : INT\nVAR s : STRING; END_VAR\n    s := 'http://x (* y *) /* z */';\nEND_FUNCTION\n",
        "FUNCTION f : INT\n    f := 1 + (* one *) 2 /* two */ - 3;\nEND_FUNCTION\n",
        "FUNCTION f : INT\nVAR x : ; (* in an error *) END_VAR\n    x := ;\nEND_FUNCTION\n",
        "// a\n// b\n\n// c\nFUNCTION f : INT\nEND_FUNCTION\n",
        "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n",
    ];
    for snippet in snippets {
        sources.push((snippet.to_string(), snippet.to_string()));
    }

    let mut total = 0;
    for (name, source) in sources {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///source.st").unwrap();
        let file = File::from_string()
            .db(&db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(source)
            .call()
            .unwrap();
        db.add_file(file).unwrap();
        let found = comments(&db, file);
        assert_eq!(found, &found_by_a_query(file.document(&db)), "{name}");
        total += found.len();
    }
    assert!(total > 300, "only {total} comments in the standard library");
}
