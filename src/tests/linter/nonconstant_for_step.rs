use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A variable step cannot prove its sign, and the loop's direction was
/// already decided at compile time (ascending) — a negative runtime value
/// will not run the loop backwards.
#[rstest]
fn variable_step_is_flagged(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT; s : INT; END_VAR
            s := 2;
            FOR i := 1 TO 10 BY s DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "nonconstant-for-step"), @r"
    [L0320] Warning: non-constant FOR step
       ,-[ file:///test0.st:5:33 ]
       |
     5 |             FOR i := 1 TO 10 BY s DO
       |                                 |
       |                                 `-- the BY step is not a compile-time literal: the loop's direction is decided at compile time (ascending), so a negative value at runtime will not run the loop backwards
       |
       | Note: lint rule: nonconstant-for-step
    ---'
    ");
}

/// Literal steps — signed, typed, or defaulted — are all clean.
#[rstest]
fn literal_steps_are_clean(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT; END_VAR
            FOR i := 1 TO 10 DO
                test := test + 1;
            END_FOR;
            FOR i := 1 TO 10 BY 2 DO
                test := test + 1;
            END_FOR;
            FOR i := 10 TO 1 BY -1 DO
                test := test + 1;
            END_FOR;
            FOR i := 1 TO 10 BY INT#2 DO
                test := test + 1;
            END_FOR;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "nonconstant-for-step"), @r"");
}
