use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn step_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 10 BY 0 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-zero-step"), @r"
    [L0131] Warning: FOR loop with zero step
       ,-[ file:///test0.st:4:25 ]
       |
     4 |     FOR i := 0 TO 10 BY 0 DO
       |                         |
       |                         `-- FOR loop step is 0, loop will never terminate
       |
       | Note: lint rule: for-zero-step
    ---'
    ");
}

#[rstest]
fn step_nonzero_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 10 BY 2 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-zero-step"), @r"");
}

#[rstest]
fn no_step_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 10 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-zero-step"), @r"");
}

#[rstest]
fn step_typed_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 10 BY INT#0 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "for-zero-step"), @r"
    [L0131] Warning: FOR loop with zero step
       ,-[ file:///test0.st:4:25 ]
       |
     4 |     FOR i := 0 TO 10 BY INT#0 DO
       |                         ^^|^^
       |                           `---- FOR loop step is 0, loop will never terminate
       |
       | Note: lint rule: for-zero-step
    ---'
    ");
}
