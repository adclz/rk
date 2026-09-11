use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"");
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
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
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
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

/// Two instances of one block share the declaration of the field they expose,
/// so `a.Q AND b.Q` used to read as one place named twice. The rule that
/// catches `x AND x` must compare the whole path, not where it ends.
#[rstest]
fn different_instances_of_the_same_field_are_not_identical(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Edge
VAR_OUTPUT
    Q : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Probe
VAR
    a_T : Edge;
    b_T : Edge;
    ok : BOOL;
END_VAR
    ok := a_T.Q AND b_T.Q;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"");
}

/// The same field of the SAME instance still is.
#[rstest]
fn one_instance_twice_is_identical(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Edge
VAR_OUTPUT
    Q : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Probe
VAR
    a_T : Edge;
    ok : BOOL;
END_VAR
    ok := a_T.Q AND a_T.Q;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
        ,-[ file:///test0.st:13:11 ]
        |
     13 |     ok := a_T.Q AND a_T.Q;
        |           ^^^^^^^|^^^^^^^
        |                  `--------- identical expressions on both sides of 'AND', result is always the same as either operand
        |
        | Note: lint rule: identical-sub-expr
    ----'
    ");
}

/// One word, two bits: one declaration and two places.
#[rstest]
fn different_bits_of_one_word_are_not_identical(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    w : WORD;
END_VAR
    test := w.0 AND w.1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"");
}

/// The same bit twice still is.
#[rstest]
fn one_bit_twice_is_identical(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    w : WORD;
END_VAR
    test := w.0 AND w.0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "identical-sub-expr"), @r"
    [L0108] Warning: identical subexpressions
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := w.0 AND w.0;
       |             ^^^^^|^^^^^
       |                  `------- identical expressions on both sides of 'AND', result is always the same as either operand
       |
       | Note: lint rule: identical-sub-expr
    ---'
    ");
}
