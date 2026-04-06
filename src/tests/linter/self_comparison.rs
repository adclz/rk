use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn eq_self(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
END_VAR
    test := x = x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"
    [L0120] Warning: self-comparison
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x = x;
       |             ^^|^^
       |               `---- 'x' is compared to itself with '=', result is always TRUE
       |
       | Note: lint rule: self-comparison
    ---'
    ");
}

#[rstest]
fn ne_self(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
END_VAR
    test := x <> x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"
    [L0120] Warning: self-comparison
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x <> x;
       |             ^^^|^^
       |                `---- 'x' is compared to itself with '<>', result is always FALSE
       |
       | Note: lint rule: self-comparison
    ---'
    ");
}

#[rstest]
fn gt_self(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
END_VAR
    test := x > x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"
    [L0120] Warning: self-comparison
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x > x;
       |             ^^|^^
       |               `---- 'x' is compared to itself with '>', result is always FALSE
       |
       | Note: lint rule: self-comparison
    ---'
    ");
}

#[rstest]
fn le_self(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
END_VAR
    test := x <= x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"
    [L0120] Warning: self-comparison
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x <= x;
       |             ^^^|^^
       |                `---- 'x' is compared to itself with '<=', result is always TRUE
       |
       | Note: lint rule: self-comparison
    ---'
    ");
}

#[rstest]
fn if_condition_self(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x = x THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"
    [L0120] Warning: self-comparison
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF x = x THEN
       |        ^^|^^
       |          `---- 'x' is compared to itself with '=', result is always TRUE
       |
       | Note: lint rule: self-comparison
    ---'
    ");
}

#[rstest]
fn no_warning_different_vars(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
    y : INT;
END_VAR
    test := x = y;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"");
}

#[rstest]
fn no_warning_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
END_VAR
    test := x = 5;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-comparison"), @r"");
}
