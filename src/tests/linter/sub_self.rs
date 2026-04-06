use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn x_minus_x(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    test := x - x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "sub-self"), @r"
    [L0126] Warning: subtraction from self
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := x - x;
       |             ^^|^^
       |               `---- 'x' is subtracted from itself, result is always 0
       |
       | Note: lint rule: sub-self
    ---'
    ");
}

#[rstest]
fn nested_in_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; y : INT; END_VAR
    test := y + (x - x);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "sub-self"), @r"
    [L0126] Warning: subtraction from self
       ,-[ file:///test0.st:4:18 ]
       |
     4 |     test := y + (x - x);
       |                  ^^|^^
       |                    `---- 'x' is subtracted from itself, result is always 0
       |
       | Note: lint rule: sub-self
    ---'
    ");
}

#[rstest]
fn no_warning_different_vars(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; y : INT; END_VAR
    test := x - y;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "sub-self"), @r"");
}

#[rstest]
fn no_warning_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    test := x - 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "sub-self"), @r"");
}
