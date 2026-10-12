// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [L0101] Warning: unreachable code
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
    [L0101] Warning: unreachable code
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
    [L0101] Warning: unreachable code
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
    [L0101] Warning: unreachable code
       ,-[ file:///test0.st:9:13 ]
       |
     9 |             x := 2;
       |             ^^^|^^
       |                `---- unreachable statement
       |
       | Note: lint rule: dead-code
    ---'
    [L0101] Warning: unreachable code
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

/// A `__TRY` ends its block when its body does and its `__CATCH` does
/// too, or when its `__FINALLY` does. A `__CATCH` that runs on leaves the
/// code after the `__TRY` alive, whatever the body does.
#[rstest]
fn the_code_after_a_try_no_path_leaves(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION both_return : INT
    __TRY
        RETURN;
    __CATCH
        RETURN;
    __END_TRY;
    both_return := 1;
END_FUNCTION

FUNCTION raised_with_no_catch : INT
    __TRY
        __RAISE('x');
        raised_with_no_catch := 1;
    __FINALLY
        raised_with_no_catch := 2;
    __END_TRY;
    raised_with_no_catch := 3;
END_FUNCTION

FUNCTION finally_raises : INT
    __TRY
        finally_raises := 1;
    __FINALLY
        __RAISE('y');
    __END_TRY;
    finally_raises := 2;
END_FUNCTION

FUNCTION catch_runs_on : INT
    __TRY
        __RAISE('z');
    __CATCH
        catch_runs_on := 1;
    __END_TRY;
    catch_runs_on := 2;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "dead-code"), @r"
    [L0101] Warning: unreachable code
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     both_return := 1;
       |     ^^^^^^^^|^^^^^^^
       |             `--------- unreachable statement
       |
       | Note: lint rule: dead-code
    ---'
    [L0101] Warning: unreachable code
        ,-[ file:///test0.st:18:5 ]
        |
     18 |     raised_with_no_catch := 3;
        |     ^^^^^^^^^^^^|^^^^^^^^^^^^
        |                 `-------------- unreachable statement
        |
        | Note: lint rule: dead-code
    ----'
    [L0101] Warning: unreachable code
        ,-[ file:///test0.st:14:9 ]
        |
     14 |         raised_with_no_catch := 1;
        |         ^^^^^^^^^^^^|^^^^^^^^^^^^
        |                     `-------------- unreachable statement
        |
        | Note: lint rule: dead-code
    ----'
    [L0101] Warning: unreachable code
        ,-[ file:///test0.st:27:5 ]
        |
     27 |     finally_raises := 2;
        |     ^^^^^^^^^|^^^^^^^^^
        |              `----------- unreachable statement
        |
        | Note: lint rule: dead-code
    ----'
    ");
}
