use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn missing_identifier(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE 
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:10 ]
       |
     2 | NAMESPACE
       |          | 
       |          `- Syntax error: Missing 'identifier'
       |          | 
       |          `- add missing identifier here
    ---'
    ");
}

#[rstest]
fn missing_end_keyword(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION myFunc : INT


    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:26 ]
       |
     2 |     FUNCTION myFunc : INT
       |                          | 
       |                          `- Syntax error: Missing 'END_FUNCTION'
       |                          | 
       |                          `- add missing END_FUNCTION here
    ---'
    ");
}

#[rstest]
fn unexpected_symbol(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE test ;
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:16 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:19 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:32 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:29 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:9 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:5 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:15 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     fn.THIS.p := 5
       |        ^^|^  
       |          `--- 'this' is not valid in this context
    ---'
    Error: 
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     THIS.a := 0
       |     ^^^|^^  
       |        `---- invalid assignment: no item 'a' in scope
    ---'
    ");
}

#[rstest]
fn empty_right_hand_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a := 
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:7 ]
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:37 ]
       |
     4 |     ml : ARRAY [0..2] OF TON := [10(call(IN := 5, OUT => OUT))]
       |                                     ^^^^^^^^^^^^|^^^^^^^^^^^^  
       |                                                 `-------------- function call in initialization expression is not allowed
    ---'
    ");
}

#[rstest]
fn missing_dot_in_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a = 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a = 0;
       |       ^|^  
       |        `--- '=' is not a valid assignment sign
       | 
       | Help: replace '=' with ':='
    ---'
    ");
}

#[rstest]
fn missing_equal_in_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a : 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a : 0;
       |       ^|^  
       |        `--- ':' is not a valid assignment sign
       | 
       | Help: replace ':' with ':='
    ---'
    ");
}

#[rstest]
fn missing_dot_in_for_list(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR i : INT END_VAR
	FOR i = 0 TO 10 END_FOR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     FOR i = 0 TO 10 END_FOR
       |           |  
       |           `-- '=' is not a valid assignment sign
       | 
       | Help: replace '=' with ':='
    ---'
    ");
}

#[rstest]
fn missing_equal_in_for_list(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR i : INT END_VAR
	FOR i : 0 TO 10 END_FOR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     FOR i : 0 TO 10 END_FOR
       |           |  
       |           `-- ':' is not a valid assignment sign
       | 
       | Help: replace ':' with ':='
    ---'
    ");
}
