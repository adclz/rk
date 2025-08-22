use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{file::File, tracked::get_ast, BaseDatabase, FileManager},
    lsp_types,
};
use hir::check::lexer::add_fixes_to_parse_errors;
use db::RootDatabase;

#[test]
fn missing_identifier() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE 
END_NAMESPACE"#;

    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
    let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

    assert_eq!(lexer_errors.len(), 1);
    assert_eq!(
        lexer_errors[0].diagnostic.message,
        "Syntax error: Missing 'identifier'"
    );
}

#[test]
fn unexpected_token() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE ns :
END_NAMESPACE"#;

    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
    let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

    assert_eq!(lexer_errors.len(), 1);
    assert_eq!(
        lexer_errors[0].diagnostic.message,
        "Unexpected token(s): ':'"
    );
}

#[test]
fn reserved_keyword() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE first
    FUNCTION

            FUNCTION

    END_FUNCTION
END_NAMESPACE"#;

    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
    let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

    assert_eq!(lexer_errors.len(), 1);
    assert_eq!(
        lexer_errors[0].diagnostic.message,
        "FUNCTION is a reserved keyword that is not valid in this context"
    );
}

#[test]
fn reserved_keyword_in_expression() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE first
    FUNCTION

            FUNCTION := something

    END_FUNCTION
END_NAMESPACE"#;

    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
    let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

    assert_eq!(lexer_errors.len(), 1);
    assert_eq!(
        lexer_errors[0].diagnostic.message,
        "FUNCTION is a reserved keyword that is not valid in this context"
    );
}
