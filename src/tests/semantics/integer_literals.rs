use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_bool_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: BOOL := 256;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: BOOL := 256;
       |         ^^|^  ^^|^    ^|^  
       |           `---------------- 'test' is declared here
       |                 |      |   
       |                 `---------- type defined here
       |                        |   
       |                        `--- invalid value initializer: invalid BOOL literal
    ---'
    ");
}

#[rstest]
fn invalid_u8(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: USINT := ULINT#2;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test: USINT := ULINT#2;
       |         ^^|^  ^^|^^    ^^^|^^^  
       |           `--------------------- 'test' is declared here
       |                 |         |     
       |                 `--------------- type defined here
       |                           |     
       |                           `----- invalid value initializer: invalid USINT literal
    ---'
    ");
}

#[rstest]
fn invalid_u16(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: UINT := ULINT#2;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: UINT := ULINT#2;
       |         ^^|^  ^^|^    ^^^|^^^  
       |           `-------------------- 'test' is declared here
       |                 |        |     
       |                 `-------------- type defined here
       |                          |     
       |                          `----- invalid value initializer: invalid UINT literal
    ---'
    ");
}

#[rstest]
fn invalid_u32(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: UDINT := ULINT#2;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test: UDINT := ULINT#2;
       |         ^^|^  ^^|^^    ^^^|^^^  
       |           `--------------------- 'test' is declared here
       |                 |         |     
       |                 `--------------- type defined here
       |                           |     
       |                           `----- invalid value initializer: invalid UDINT literal
    ---'
    ");
}

#[rstest]
fn invalid_u64(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: ULINT := REAL#0.0;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test: ULINT := REAL#0.0;
       |         ^^|^  ^^|^^    ^^^^|^^^  
       |           `---------------------- 'test' is declared here
       |                 |          |     
       |                 `---------------- type defined here
       |                            |     
       |                            `----- invalid value initializer: invalid ULINT literal
    ---'
    ");
}

#[rstest]
fn invalid_i8(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: SINT := REAL#0.0;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: SINT := REAL#0.0;
       |         ^^|^  ^^|^    ^^^^|^^^  
       |           `--------------------- 'test' is declared here
       |                 |         |     
       |                 `--------------- type defined here
       |                           |     
       |                           `----- invalid value initializer: invalid SINT literal
    ---'
    ");
}

#[rstest]
fn invalid_i16(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT := REAL#0.0;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:22 ]
       |
     4 |         test: INT := REAL#0.0;
       |         ^^|^  ^|^    ^^^^|^^^  
       |           `-------------------- 'test' is declared here
       |                |         |     
       |                `--------------- type defined here
       |                          |     
       |                          `----- invalid value initializer: invalid INT literal
    ---'
    ");
}

#[rstest]
fn invalid_i32(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: DINT := REAL#0.0;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: DINT := REAL#0.0;
       |         ^^|^  ^^|^    ^^^^|^^^  
       |           `--------------------- 'test' is declared here
       |                 |         |     
       |                 `--------------- type defined here
       |                           |     
       |                           `----- invalid value initializer: invalid DINT literal
    ---'
    ");
}

#[rstest]
fn invalid_i64(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: LINT := REAL#0.0;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: LINT := REAL#0.0;
       |         ^^|^  ^^|^    ^^^^|^^^  
       |           `--------------------- 'test' is declared here
       |                 |         |     
       |                 `--------------- type defined here
       |                           |     
       |                           `----- invalid value initializer: invalid LINT literal
    ---'
    ");
}
