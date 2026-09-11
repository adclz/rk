use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn empty_for(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-loop-body"), @r"
    [L0308] Hint: empty loop body
       ,-[ file:///test0.st:6:5 ]
       |
     6 | ,->     FOR i := 0 TO 10 DO
     7 | |->     END_FOR;
       | |
       | `------------------ FOR loop has no statements
       |
       |     Note: lint rule: empty-loop-body
    ---'
    ");
}

#[rstest]
fn empty_while(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : BOOL;
END_VAR
    WHILE x DO
    END_WHILE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-loop-body"), @r"
    [L0308] Hint: empty loop body
       ,-[ file:///test0.st:6:11 ]
       |
     6 |     WHILE x DO
       |           |
       |           `-- WHILE loop has no statements
       |
       | Note: lint rule: empty-loop-body
    ---'
    ");
}

#[rstest]
fn empty_repeat(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : BOOL;
END_VAR
    REPEAT
    UNTIL x
    END_REPEAT;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-loop-body"), @r"
    [L0308] Hint: empty loop body
       ,-[ file:///test0.st:7:11 ]
       |
     7 |     UNTIL x
       |           |
       |           `-- REPEAT loop has no statements
       |
       | Note: lint rule: empty-loop-body
    ---'
    ");
}

#[rstest]
fn non_empty_for(mut with_db: RootDatabase) {
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-loop-body"), @r"");
}
