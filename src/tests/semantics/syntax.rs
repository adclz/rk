use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn old_syntax_config(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION cfg
    VAR_GLOBAL w: UINT; END_VAR
    
    RESOURCE STATION_1 ON PROCESSOR_TYPE_1
        VAR_GLOBAL z1: BYTE; END_VAR
    END_RESOURCE

END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:1 ]
       |
     2 | ,-> CONFIGURATION cfg
       : :   
     9 | |-> END_CONFIGURATION
       | |                      
       | `---------------------- deprecated syntax for CONFIG declaration
       | |   
       | |   Note: use config.toml to declare resources and tasks
    ---'
    ");
}

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
CLASS b
END_CLASS

FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:5:19 ]
       |
     5 | FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b
       |                   ^^^^^^|^^^^^  
       |                         `------- implements must be declared after extends
    ---'
    ");
}

#[rstest]
fn implements_multiple_times(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE a
END_INTERFACE

INTERFACE b
END_INTERFACE

FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:32 ]
       |
     8 | FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b
       |                                ^^^^^^|^^^^^  
       |                                      `------- multiple implements declarations
    ---'
    ");
}

#[rstest]
fn extends_multiple_times(mut with_db: RootDatabase) {
    let source = r#"
CLASS a
END_CLASS

CLASS b
END_CLASS

FUNCTION_BLOCK fn EXTENDS a EXTENDS b

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:29 ]
       |
     8 | FUNCTION_BLOCK fn EXTENDS a EXTENDS b
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
fn unexpected_this_in_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn.THIS.p := 5
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     fn.THIS.p := 5
       |        ^^|^  
       |          `--- 'THIS' is not valid in this context
    ---'
    ");
}

#[rstest]
fn unexpected_super_in_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn.SUPER.p := 5
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     fn.SUPER.p := 5
       |        ^^|^^  
       |          `---- 'SUPER' is not valid in this context
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

#[rstest]
fn class_variables_before_method(mut with_db: RootDatabase) {
    let source = r#"
CLASS base

    METHOD PROTECTED myProtectedMethod END_METHOD

    VAR
        obj: Base;
    END_VAR

    VAR
        obj: Base;
    END_VAR

END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:6:5 ]
        |
      6 | ,->     VAR
        : :   
     12 | |->     END_VAR
        | |                 
        | `----------------- class variable declarations must appear before methods
    ----'
    ");
}

#[rstest]
fn fb_variables_before_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1

    METHOD PROTECTED myProtectedMethod END_METHOD

    VAR
        obj: Base;
    END_VAR

    VAR
        obj: Base;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:6:5 ]
        |
      6 | ,->     VAR
        : :   
     12 | |->     END_VAR
        | |                 
        | `----------------- FB variable declarations must appear before methods
    ----'
    ");
}
