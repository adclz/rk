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
    [W0101] Warning: unused code
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             x : INT;
       |             ^^^|^^^
       |                `----- unused variable 'x'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_x'
    ---'
    [W0107] Warning: unreachable code
       ,-[ file:///test0.st:8:13 ]
       |
     8 |             x := 2;
       |             ^^^|^^
       |                `---- unreachable statement
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
    [W0107] Warning: unreachable code
       ,-[ file:///test0.st:9:17 ]
       |
     9 |                 test := 0;
       |                 ^^^^|^^^^
       |                     `------ unreachable statement
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
    [W0107] Warning: unreachable code
       ,-[ file:///test0.st:8:17 ]
       |
     8 |                 test := i;
       |                 ^^^^|^^^^
       |                     `------ unreachable statement
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
    [W0113] Advice: unnecessary ELSE
       ,-[ file:///test0.st:9:17 ]
       |
     9 |                 test := x;
       |                 ^^^^|^^^^
       |                     `------ unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE
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
    [W0101] Warning: unused code
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             x : INT;
       |             ^^^|^^^
       |                `----- unused variable 'x'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_x'
    ---'
    [W0101] Warning: unused code
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             ^^^|^^^
       |                `----- unused variable 'y'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_y'
    ---'
    [W0107] Warning: unreachable code
       ,-[ file:///test0.st:9:13 ]
       |
     9 |             x := 2;
       |             ^^^|^^
       |                `---- unreachable statement
    ---'
    [W0107] Warning: unreachable code
        ,-[ file:///test0.st:10:13 ]
        |
     10 |             y := 3;
        |             ^^^|^^
        |                `---- unreachable statement
    ----'
    ");
}
