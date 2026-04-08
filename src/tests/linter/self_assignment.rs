use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn simple_self_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := x;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0309] Warning: self-assignment
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     x := x;
       |     ^^^|^^
       |        `---- variable 'x' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

#[rstest]
fn no_warning_different_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
    y : INT;
END_VAR
    x := y;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"");
}

#[rstest]
fn no_warning_expression_rhs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := x + 1;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"");
}

#[rstest]
fn self_assignment_in_if_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
    flag : BOOL;
END_VAR
    IF flag THEN
        x := x;
    END_IF;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0309] Warning: self-assignment
       ,-[ file:///test0.st:8:9 ]
       |
     8 |         x := x;
       |         ^^^|^^
       |            `---- variable 'x' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

#[rstest]
fn self_assignment_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
VAR
    counter : INT;
END_VAR
    counter := counter;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0309] Warning: self-assignment
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     counter := counter;
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- variable 'counter' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}
