use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn statement_after_return(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            test := 1;
            RETURN;
            x := 2;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             x : INT;
       |             ^^^|^^^
       |                `----- unused variable 'x'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_x'
       |
       | Note 2: lint rule: unused-variable
    ---'
    [L0107] Warning: unreachable code
       ,-[ file:///test0.st:8:13 ]
       |
     8 |             x := 2;
       |             ^^^|^^
       |                `---- unreachable statement
       |
       | Note: lint rule: dead-code
    ---'
    ");
}

#[rstest]
fn return_at_end_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            x := 1;
            test := x;
            RETURN;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn statement_after_exit_in_loop(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 1 TO 10 DO
                test := i;
                EXIT;
                test := 0;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0107] Warning: unreachable code
       ,-[ file:///test0.st:9:17 ]
       |
     9 |                 test := 0;
       |                 ^^^^|^^^^
       |                     `------ unreachable statement
       |
       | Note: lint rule: dead-code
    ---'
    ");
}

#[rstest]
fn statement_after_continue_in_loop(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 1 TO 10 DO
                CONTINUE;
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0107] Warning: unreachable code
       ,-[ file:///test0.st:8:17 ]
       |
     8 |                 test := i;
       |                 ^^^^|^^^^
       |                     `------ unreachable statement
       |
       | Note: lint rule: dead-code
    ---'
    ");
}

#[rstest]
fn code_in_different_if_branch_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            IF x > 0 THEN
                RETURN;
            ELSE
                test := x;
            END_IF;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0113] Advice: unnecessary ELSE
       ,-[ file:///test0.st:9:17 ]
       |
     9 |                 test := x;
       |                 ^^^^|^^^^
       |                     `------ unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE
       |
       | Note: lint rule: unnecessary-else
    ---'
    ");
}

#[rstest]
fn multiple_statements_after_return(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            test := 1;
            RETURN;
            x := 2;
            y := 3;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0101] Warning: unused code
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             x : INT;
       |             ^^^|^^^
       |                `----- unused variable 'x'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_x'
       |
       | Note 2: lint rule: unused-variable
    ---'
    [L0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             ^^^|^^^
       |                `----- unused variable 'y'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_y'
       |
       | Note 2: lint rule: unused-variable
    ---'
    [L0107] Warning: unreachable code
       ,-[ file:///test0.st:9:13 ]
       |
     9 |             x := 2;
       |             ^^^|^^
       |                `---- unreachable statement
       |
       | Note: lint rule: dead-code
    ---'
    [L0107] Warning: unreachable code
        ,-[ file:///test0.st:10:13 ]
        |
     10 |             y := 3;
        |             ^^^|^^
        |                `---- unreachable statement
        |
        | Note: lint rule: dead-code
    ----'
    ");
}
