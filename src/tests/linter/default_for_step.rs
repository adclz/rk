use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn step_of_one(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 BY 1 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "default-for-step"), @r"
    [L0215] Hint: redundant FOR loop step
       ,-[ file:///test0.st:6:25 ]
       |
     6 |     FOR i := 0 TO 10 BY 1 DO
       |                         |
       |                         `-- FOR loop step of 1 is the default and can be omitted
       |
       | Note: lint rule: default-for-step
    ---'
    ");
}

#[rstest]
fn step_of_two(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 BY 2 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "default-for-step"), @r"");
}

#[rstest]
fn no_step(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "default-for-step"), @r"");
}

#[rstest]
fn step_of_negative_one(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 10 TO 0 BY -1 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "default-for-step"), @r"");
}
