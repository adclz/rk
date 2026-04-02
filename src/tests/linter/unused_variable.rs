use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn unused_local_variable(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            fn1 := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             ^^^|^^^
       |                `----- unused variable 'y'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_y'
    ---'
    ");
}

#[rstest]
fn unused_variable_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR
            x : INT;
            y : INT;
        END_VAR
            x := 1;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             ^^^|^^^
       |                `----- unused variable 'y'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_y'
    ---'
    ");
}

#[rstest]
fn unused_variable_in_program(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR
            x : INT;
            y : INT;
        END_VAR
            x := 1;
        END_PROGRAM
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             ^^^|^^^
       |                `----- unused variable 'y'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_y'
    ---'
    ");
}

#[rstest]
fn all_variables_used(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            fn1 := x + y;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn output_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR_OUTPUT
            result : INT;
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0114] Warning: uninitialized output
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             result : INT;
       |             ^^^^^^|^^^^^
       |                   `------- VAR_OUTPUT 'result' is never assigned in the body
    ---'
    ");
}

#[rstest]
fn inout_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR_IN_OUT
            data : INT;
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn input_on_program_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR_INPUT
            sensor : INT;
        END_VAR
        END_PROGRAM
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn unused_input_on_function(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            fn1 := a;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             b : INT;
       |             ^^^|^^^
       |                `----- unused variable 'b'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_b'
    ---'
    ");
}

#[rstest]
fn underscore_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            _ : INT;
        END_VAR
            fn1 := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn variable_used_via_this_in_method_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
        VAR
            speed : INT;
            unused_var : INT;
        END_VAR

        METHOD start
            THIS.speed := 100;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;
    // speed is used via THIS in a method - should NOT be flagged
    // unused_var is never used anywhere - should be flagged
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             unused_var : INT;
       |             ^^^^^^^^|^^^^^^^
       |                     `--------- unused variable 'unused_var'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_unused_var'
    ---'
    ");
}
