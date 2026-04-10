use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn multiply_by_one(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x * 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := x * 1;
       |          ^^|^^
       |            `---- '* 1' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn one_multiply(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := 1 * x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := 1 * x;
       |          ^^|^^
       |            `---- '1 *' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn divide_by_one(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x / 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := x / 1;
       |          ^^|^^
       |            `---- '/ 1' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn add_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x + 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := x + 0;
       |          ^^|^^
       |            `---- '+ 0' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn zero_add(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := 0 + x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := 0 + x;
       |          ^^|^^
       |            `---- '0 +' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn subtract_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x - 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := x - 0;
       |          ^^|^^
       |            `---- '- 0' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn real_multiply_by_one(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : REAL
VAR x : REAL; END_VAR
    x := x * 1.0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := x * 1.0;
       |          ^^^|^^^
       |             `----- '* 1' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn typed_literal_one(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x * INT#1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"
    [L0312] Warning: identity operation
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     x := x * INT#1;
       |          ^^^^|^^^^
       |              `------ '* 1' has no effect, the result is always the same as the other operand
       |
       | Note: lint rule: identity-operation
    ---'
    ");
}

#[rstest]
fn no_warning_multiply_by_two(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x * 2;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"");
}

#[rstest]
fn no_warning_add_nonzero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    x := x + 5;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identity-operation"), @r"");
}
