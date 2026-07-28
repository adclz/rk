use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// The counter wraps at the type width
/// before the exit check can fail, so the loop runs forever — codegen
/// deliberately wraps sub-width arithmetic, so this is REAL runtime
/// behavior, not a theoretical concern.
#[rstest]
fn usint_to_255_never_terminates(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : USINT; END_VAR
            FOR i := 0 TO 255 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-bound-at-type-limit"), @r#"
    [L0319] Warning: FOR loop never terminates
       ,-[ file:///test0.st:4:27 ]
       |
     4 |             FOR i := 0 TO 255 DO
       |                           ^|^
       |                            `--- this FOR loop never terminates: the bound 255 is USINT's own limit, so the counter wraps before the exit check can fail
       |
       | Note: lint rule: for-bound-at-type-limit
    ---'
    "#);
}

/// One below the limit terminates fine — no warning.
#[rstest]
fn usint_to_254_is_clean(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : USINT; END_VAR
            FOR i := 0 TO 254 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-bound-at-type-limit"), @r"");
}

/// Signed types wrap at their own maximum the same way.
#[rstest]
fn int_to_32767_never_terminates(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT; END_VAR
            FOR i := 0 TO 32767 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-bound-at-type-limit"), @r"
    [L0319] Warning: FOR loop never terminates
       ,-[ file:///test0.st:4:27 ]
       |
     4 |             FOR i := 0 TO 32767 DO
       |                           ^^|^^
       |                             `---- this FOR loop never terminates: the bound 32767 is INT's own limit, so the counter wraps before the exit check can fail
       |
       | Note: lint rule: for-bound-at-type-limit
    ---'
    ");
}

/// Descending to the signed MINIMUM wraps at the bottom.
#[rstest]
fn int_descending_to_min_never_terminates(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT; END_VAR
            FOR i := 0 TO -32768 BY -1 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-bound-at-type-limit"), @r"
    [L0319] Warning: FOR loop never terminates
       ,-[ file:///test0.st:4:27 ]
       |
     4 |             FOR i := 0 TO -32768 BY -1 DO
       |                           ^^^|^^
       |                              `---- this FOR loop never terminates: the bound -32768 is INT's own limit, so the counter wraps before the exit check can fail
       |
       | Note: lint rule: for-bound-at-type-limit
    ---'
    ");
}

/// A SUBRANGE counter wraps at its BASE type's width, not at the subrange
/// bound — `TO 100` on `INT (0..100)` terminates and stays clean.
#[rstest]
fn subrange_bound_is_not_the_type_limit(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..100); END_TYPE

        FUNCTION test : INT
        VAR i : Small; END_VAR
            FOR i := 0 TO 100 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-bound-at-type-limit"), @r"");
}

/// A typed-literal bound (`USINT#255`) is folded and flagged too.
#[rstest]
fn typed_literal_bound_is_flagged(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : USINT; END_VAR
            FOR i := 0 TO USINT#255 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-bound-at-type-limit"), @r##"
    [L0319] Warning: FOR loop never terminates
       ,-[ file:///test0.st:4:27 ]
       |
     4 |             FOR i := 0 TO USINT#255 DO
       |                           ^^^^|^^^^
       |                               `------ this FOR loop never terminates: the bound 255 is USINT's own limit, so the counter wraps before the exit check can fail
       |
       | Note: lint rule: for-bound-at-type-limit
    ---'
    "##);
}
