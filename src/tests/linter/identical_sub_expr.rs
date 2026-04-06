use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn and_same_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x AND x;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0121] Warning: identical subexpressions
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x AND x;
       |             ^^^|^^^
       |                `----- identical expressions on both sides of 'AND', result is always the same as either operand
       |
       | Note: lint rule: identical-sub-expr
    ---'
    ");
}

#[rstest]
fn or_same_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x OR x;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0121] Warning: identical subexpressions
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x OR x;
       |             ^^^|^^
       |                `---- identical expressions on both sides of 'OR', result is always the same as either operand
       |
       | Note: lint rule: identical-sub-expr
    ---'
    ");
}

#[rstest]
fn xor_same_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x XOR x;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0121] Warning: identical subexpressions
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x XOR x;
       |             ^^^|^^^
       |                `----- identical expressions on both sides of 'XOR', result is always FALSE
       |
       | Note: lint rule: identical-sub-expr
    ---'
    ");
}

#[rstest]
fn no_warning_different_vars(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
    y : BOOL;
END_VAR
    test := x AND y;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn if_condition_identical(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF flag OR flag THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0121] Warning: identical subexpressions
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF flag OR flag THEN
       |        ^^^^^^|^^^^^
       |              `------- identical expressions on both sides of 'OR', result is always the same as either operand
       |
       | Note: lint rule: identical-sub-expr
    ---'
    ");
}

#[rstest]
fn not_x_and_not_x(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := NOT x AND NOT x;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0121] Warning: identical subexpressions
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := NOT x AND NOT x;
       |             ^^^^^^^|^^^^^^^
       |                    `--------- identical expressions on both sides of 'AND', result is always the same as either operand
       |
       | Note: lint rule: identical-sub-expr
    ---'
    ");
}
