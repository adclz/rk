use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types, tree_sitter,
};
use db::RootDatabase;
use ide_proto::comment_index::comment_index;

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
    let comment = index.find_nearby_comment(document, &range).unwrap();

    assert_eq!(comment.to_string(document), "This is a single line comment");
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

    let comment = index.find_nearby_comment(document, &range).unwrap();
    assert_eq!(comment.to_string(document), "right side comment");
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
    let comment = index.find_nearby_comment(document, &range).unwrap();
    assert_eq!(comment.to_string(document), "Separated by blank lines");
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
    let comment = index.find_nearby_comment(document, &range).unwrap();

    assert_eq!(comment.to_string(document), "This is a C-style comment");
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
    let comment = index.find_nearby_comment(document, &range).unwrap();
    assert_eq!(
        comment.to_string(document),
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
    let comment = index.find_nearby_comment(document, &range).unwrap();
    assert_eq!(comment.to_string(document), "NOT NESTED\n  (* NESTED *)");
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
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let index = comment_index(&db, file);

    // Comment should only be picked *once* for x
    // y should not pick it up because the comment is to the right of x
    assert!(index.map.len() == 1, "Expected one comment in the index");
}
