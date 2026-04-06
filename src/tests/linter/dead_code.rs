use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @r"
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @r"");
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @r"
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @r"
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @"");
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @r"
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
