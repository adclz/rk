use ariadne::CharSet;
use ariadne::Config;
use ariadne::Source;
use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use insta::assert_snapshot;
use rstest::{fixture, rstest};

#[fixture]
fn with_db() -> RootDatabase {
    RootDatabase::default()
}

fn no_color_and_ascii() -> Config {
    Config::default()
        .with_color(false)
        // Using Ascii so that the inline snapshots display correctly
        // even with fonts where characters like '┬' take up more space.
        .with_char_set(CharSet::Ascii)
}

fn test_diagnostic(db: &mut RootDatabase, source: &str) -> String {
    let url = Url::parse("file:///test.st").unwrap();

    let file = File::from_string()
        .db(db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let mut cache = vec![];
    diagnostics_for_file(db, file)[0]
        .create_report(db, file, Some(no_color_and_ascii()))
        .write(
            (url.as_str(), Source::from(file.document(db).as_str())),
            &mut cache,
        )
        .unwrap();

    String::from_utf8(cache).unwrap()
}

#[rstest]
fn missing_identifier(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE 
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:2:10 ]
       |
     2 | NAMESPACE
       |          | 
       |          `- Syntax error: Missing 'identifier'
       | 
       | Help: add missing identifier here
    ---'
    ");
}

#[rstest]
fn missing_end_keyword(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION myFunc : INT


    "#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:2:26 ]
       |
     2 |     FUNCTION myFunc : INT
       |                          | 
       |                          `- Syntax error: Missing 'END_FUNCTION'
       | 
       | Help: add missing END_FUNCTION here
    ---'
    ");
}


#[rstest]
fn unexpected_symbol(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE test ;
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:2:16 ]
       |
     2 | NAMESPACE test ;
       |                |  
       |                `-- Unexpected token(s): ';'
    ---'
    ");
}


#[rstest]
fn implements_before_extends(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b
    
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:2:19 ]
       |
     2 | FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b
       |                   ^^^^^^|^^^^^  
       |                         `------- implements must be declared after extends
    ---'
    ");
}

#[rstest]
fn implements_multiple_times(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b
    
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:2:32 ]
       |
     2 | FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b
       |                                ^^^^^^|^^^^^  
       |                                      `------- multiple implements declarations
    ---'
    ");
}

#[rstest]
fn extends_multiple_times(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn EXTENDS a EXTENDS b
    
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:2:29 ]
       |
     2 | FUNCTION_BLOCK fn EXTENDS a EXTENDS b
       |                             ^^^^|^^^^  
       |                                 `------ multiple extends declarations
    ---'
    ");
}

#[rstest]
fn variable_with_no_spec(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    VAR_INPUT
        empty
    END_VAR
    
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:4:9 ]
       |
     4 |         empty
       |         ^^|^^  
       |           `---- variable type is missing
    ---'
    ");
}

#[rstest]
fn function_call_as_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn() := 0;   
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:3:5 ]
       |
     3 |     fn() := 0;
       |     ^^|^  
       |       `--- assignment to function call is not allowed
    ---'
    ");
}

#[rstest]
fn invocation_in_expression_context(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn.m.p := THIS.m^()   
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:3:15 ]
       |
     3 |     fn.m.p := THIS.m^()
       |               ^^^^|^^^^  
       |                   `------ invocation in expression is not allowed
    ---'
    ");
}

#[rstest]
fn unexpected_this_in_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    THIS.a := 0
    fn.THIS.p := 5
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:4:8 ]
       |
     4 |     fn.THIS.p := 5
       |        ^^|^  
       |          `--- 'this' is not valid in this context
    ---'
    ");
}

#[rstest]
fn empty_right_hand_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a := 
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:3:7 ]
       |
     3 |     a :=
       |       ^|  
       |        `-- right-hand side of assignment cannot be empty
    ---'
    ");
}

#[rstest]
fn function_call_in_init_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
  VAR
    ml : ARRAY [0..2] OF TON := [10(call(IN := 5, OUT => OUT))]
  END_VAR    
    
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:4:37 ]
       |
     4 |     ml : ARRAY [0..2] OF TON := [10(call(IN := 5, OUT => OUT))]
       |                                     ^^^^^^^^^^^^|^^^^^^^^^^^^  
       |                                                 `-------------- function call in initialization expression is not allowed
    ---'
    ");
}
