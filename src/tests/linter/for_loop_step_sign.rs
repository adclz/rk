use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn ascending_with_positive_step_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 1 TO 10 BY 1 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"");
}

#[rstest]
fn descending_with_negative_step_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 10 TO 1 BY -1 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"");
}

#[rstest]
fn ascending_with_negative_step_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 1 TO 10 BY -1 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"
    [L0108] Info: FOR loop step sign mismatch
       ,-[ file:///test0.st:6:13 ]
       |
     6 | ,->             FOR i := 1 TO 10 BY -1 DO
       : :
     8 | |->             END_FOR;
       | |
       | `-------------------------- FOR loop step direction mismatches bounds direction
       |
       |     Note: lint rule: for-loop-step-sign
    ---'
    ");
}

#[rstest]
fn descending_with_positive_step_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 10 TO 1 BY 1 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"
    [L0108] Info: FOR loop step sign mismatch
       ,-[ file:///test0.st:6:13 ]
       |
     6 | ,->             FOR i := 10 TO 1 BY 1 DO
       : :
     8 | |->             END_FOR;
       | |
       | `-------------------------- FOR loop step direction mismatches bounds direction
       |
       |     Note: lint rule: for-loop-step-sign
    ---'
    ");
}

#[rstest]
fn ascending_default_step_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 1 TO 10 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"");
}

#[rstest]
fn descending_default_step_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 10 TO 1 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"
    [L0108] Info: FOR loop step sign mismatch
       ,-[ file:///test0.st:6:13 ]
       |
     6 | ,->             FOR i := 10 TO 1 DO
       : :
     8 | |->             END_FOR;
       | |
       | `-------------------------- FOR loop step direction mismatches bounds direction
       |
       |     Note: lint rule: for-loop-step-sign
    ---'
    ");
}

#[rstest]
fn equal_bounds_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            i : INT;
        END_VAR
            FOR i := 5 TO 5 BY 1 DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"");
}
