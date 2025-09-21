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
       |                        `--- invalid BOOL literal
       | 
       | Note: A BOOL literal must be either 0, 1, TRUE or FALSE
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
       |                           `----- invalid USINT literal
       | 
       | Note: A USINT literal must be an integer between 0 and 255
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
       |                          `----- invalid UINT literal
       | 
       | Note: A UINT literal must be an integer between 0 and 65535
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
       |                           `----- invalid UDINT literal
       | 
       | Note: A UDINT literal must be an integer between 0 and 4294967295
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
       |                            `----- invalid ULINT literal
       | 
       | Note: A ULINT literal must be an integer between 0 and 18446744073709551615
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
       |                           `----- invalid SINT literal
       | 
       | Note: A SINT literal must be an integer between -128 and 127
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
       |                          `----- invalid INT literal
       | 
       | Note: An INT literal must be an integer between -32768 and 32767
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
       |                           `----- invalid DINT literal
       | 
       | Note: A DINT literal must be an integer between -2147483648 and 2147483647
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
       |                           `----- invalid LINT literal
       | 
       | Note: A LINT literal must be an integer between -9223372036854775808 and 9223372036854775807
    ---'
    ");
}
