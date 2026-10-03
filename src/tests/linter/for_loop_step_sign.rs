// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [L0110] Warning: FOR loop step sign mismatch
       ,-[ file:///test0.st:6:33 ]
       |
     6 |             FOR i := 1 TO 10 BY -1 DO
       |                                 ^|
       |                                  `-- the sign of the step does not match the direction of the bounds
       |
       | Note: lint rule: for-loop-step-sign
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
    [L0110] Warning: FOR loop step sign mismatch
       ,-[ file:///test0.st:6:33 ]
       |
     6 |             FOR i := 10 TO 1 BY 1 DO
       |                                 |
       |                                 `-- the sign of the step does not match the direction of the bounds
       |
       | Note: lint rule: for-loop-step-sign
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
    [L0110] Warning: FOR loop step sign mismatch
       ,-[ file:///test0.st:6:22 ]
       |
     6 |             FOR i := 10 TO 1 DO
       |                      ^^^|^^^
       |                         `----- the sign of the step does not match the direction of the bounds
       |
       | Note: lint rule: for-loop-step-sign
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

// The bounds fold through const_int, so CONSTANT bounds are seen too - a
// literal-only match let this descending loop pass unflagged.
#[rstest]
fn constant_bounds_are_folded(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR CONSTANT LO : INT := 1; HI : INT := 10; END_VAR
        VAR i : INT; END_VAR
            FOR i := HI TO LO DO
                test := i;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"
    [L0110] Warning: FOR loop step sign mismatch
       ,-[ file:///test0.st:5:22 ]
       |
     5 |             FOR i := HI TO LO DO
       |                      ^^^^|^^^
       |                          `----- the sign of the step does not match the direction of the bounds
       |
       | Note: lint rule: for-loop-step-sign
    ---'
    ");
}

// The step folds at its type, as the counter adds it: `K + 1` on a SINT
// K = 127 is -128, so this ascending range never runs.
#[rstest]
fn step_folds_at_its_type(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR CONSTANT K : SINT := 127; END_VAR
        VAR i : DINT; END_VAR
            FOR i := 0 TO 1000 BY K + 1 DO
                test := 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-loop-step-sign"), @r"
    [L0110] Warning: FOR loop step sign mismatch
       ,-[ file:///test0.st:5:35 ]
       |
     5 |             FOR i := 0 TO 1000 BY K + 1 DO
       |                                   ^^|^^
       |                                     `---- the sign of the step does not match the direction of the bounds
       |
       | Note: lint rule: for-loop-step-sign
    ---'
    ");
}
