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
    [L0106] Warning: subtraction from self
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
    [L0106] Warning: subtraction from self
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

/// A float minus itself is NaN for a NaN or an infinity, and 0 only when
/// finite: that difference is a finiteness test, not always 0. An integer
/// minus itself is still one.
#[rstest]
fn a_float_minus_itself_is_a_finiteness_test(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            x : LREAL;
            n : DINT;
        END_VAR
            f := (x - x) <> LREAL#0.0;
            f := (n - n) <> 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "sub-self"), @r"
    [L0106] Warning: subtraction from self
       ,-[ file:///test0.st:8:19 ]
       |
     8 |             f := (n - n) <> 0;
       |                   ^^|^^
       |                     `---- 'n' is subtracted from itself, result is always 0
       |
       | Note: lint rule: sub-self
    ---'
    ");
}
