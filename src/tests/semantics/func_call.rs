use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn not_a_callable_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:5 ]
       |
     4 |         test: INT;
       |         ^^|^  ^|^  
       |           `-------- 'test' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     test();
       |     ^^|^  
       |       `--- cannot call non-callable type 'test'
       | 
       | Note: only functions, function blocks or methods can be called
    ---'
    ");
}

#[rstest]
fn unused_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL

END_FUNCTION

FUNCTION_BLOCK fb1
    fn();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Warning: 
       ,-[ file:///test0.st:7:5 ]
       |
     2 | FUNCTION fn: BOOL
       |          ^|  ^^|^  
       |           `-------- 'fn' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     fn();
       |     ^|  
       |      `-- unused return type of 'fn'
    ---'
    ");
}

#[rstest]
fn unknown_input_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        unknown := TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:9 ]
       |
     2 | ,-> FUNCTION fn
       | |            ^|  
       | |             `-- 'fn' is declared here
     3 | |-> END_FUNCTION
       | |                  
       | `------------------ type defined here
       | 
     7 |             unknown := TRUE
       |             ^^^|^^^  
       |                `----- unknown input parameter 'unknown'
    ---'
    ");
}

#[rstest]
fn unknown_output_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        unknown => TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:7:9 ]
       |
     2 | ,-> FUNCTION fn
       | |            ^|  
       | |             `-- 'fn' is declared here
     3 | |-> END_FUNCTION
       | |                  
       | `------------------ type defined here
       | 
     7 |             unknown => TRUE
       |             ^^^|^^^  
       |                `----- unknown output parameter 'unknown'
    ---'
    ");
}
