use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn equal_literal_bounds(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 5 TO 5 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-loop-bounds"), @r"
    [L0314] Info: constant FOR loop bounds
       ,-[ file:///test0.st:6:14 ]
       |
     6 |     FOR i := 5 TO 5 DO
       |              |
       |              `-- FOR loop bounds are equal (both 5), loop body executes exactly once
       |
       | Note: lint rule: constant-loop-bounds
    ---'
    ");
}

#[rstest]
fn different_literal_bounds(mut with_db: RootDatabase) {
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-loop-bounds"), @r"");
}

#[rstest]
fn zero_to_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 0 DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-loop-bounds"), @r"
    [L0314] Info: constant FOR loop bounds
       ,-[ file:///test0.st:6:14 ]
       |
     6 |     FOR i := 0 TO 0 DO
       |              |
       |              `-- FOR loop bounds are equal (both 0), loop body executes exactly once
       |
       | Note: lint rule: constant-loop-bounds
    ---'
    ");
}

#[rstest]
fn same_variable_bounds(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := n TO n DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-loop-bounds"), @r"
    [L0314] Info: constant FOR loop bounds
       ,-[ file:///test0.st:7:14 ]
       |
     7 |     FOR i := n TO n DO
       |              |
       |              `-- FOR loop bounds are equal (both n), loop body executes exactly once
       |
       | Note: lint rule: constant-loop-bounds
    ---'
    ");
}

#[rstest]
fn different_variable_bounds(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
    a : INT;
    b : INT;
END_VAR
    FOR i := a TO b DO
        test := i;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-loop-bounds"), @r"");
}
