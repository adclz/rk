// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn if_always_true(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    IF TRUE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     IF TRUE THEN
       |        ^^|^
       |          `--- IF condition is always TRUE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

#[rstest]
fn if_always_false(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    IF FALSE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     IF FALSE THEN
       |        ^^|^^
       |          `---- IF condition is always FALSE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

#[rstest]
fn while_always_true(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    WHILE TRUE DO
        test := 1;
    END_WHILE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:3:11 ]
       |
     3 |     WHILE TRUE DO
       |           ^^|^
       |             `--- WHILE condition is always TRUE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

#[rstest]
fn repeat_always_false(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    REPEAT
        test := 1;
    UNTIL FALSE
    END_REPEAT;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:5:11 ]
       |
     5 |     UNTIL FALSE
       |           ^^|^^
       |             `---- UNTIL condition is always FALSE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

#[rstest]
fn no_warning_variable_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF flag THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"");
}

#[rstest]
fn no_warning_expression_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 0 THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"");
}

#[rstest]
fn elsif_always_true(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF flag THEN
        test := 1;
    ELSIF TRUE THEN
        test := 2;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:8:11 ]
       |
     8 |     ELSIF TRUE THEN
       |           ^^|^
       |             `--- ELSIF condition is always TRUE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

#[rstest]
fn parenthesized_boolean_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    IF (TRUE) THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     IF (TRUE) THEN
       |        ^^^|^^
       |           `---- IF condition is always TRUE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

#[rstest]
fn implicit_boolean_literals(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    IF BOOL#TRUE THEN
        test := 1;
    END_IF
    IF BOOL#FALSE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     IF BOOL#TRUE THEN
       |        ^^^^|^^^^
       |            `------ IF condition is always TRUE
       |
       | Note: lint rule: constant-condition
    ---'
    [L0103] Warning: constant condition
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF BOOL#FALSE THEN
       |        ^^^^^|^^^^
       |             `------ IF condition is always FALSE
       |
       | Note: lint rule: constant-condition
    ---'
    ");
}

/// `BOOL#1` and `BOOL#0` are TRUE and FALSE: the message said the condition
/// was always `1`.
#[rstest]
#[case::one("IF BOOL#1 THEN n := 1; END_IF;", "IF condition is always TRUE")]
#[case::zero(
    "WHILE BOOL#0 DO n := 1; END_WHILE;",
    "WHILE condition is always FALSE"
)]
fn a_bool_literal_condition_is_named_by_its_value(
    mut with_db: RootDatabase,
    #[case] statement: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"
FUNCTION test : INT
VAR n : INT; END_VAR
    {statement}
    test := n;
END_FUNCTION
"#
    );
    let rendered = test_single_lint(&mut with_db, &[&source], "constant-condition");
    assert!(
        rendered.contains(message),
        "`{statement}` must say `{message}`, got:\n{rendered}"
    );
}

/// A comparison the value's type decides: an unsigned integer is never
/// below zero, a SINT never above 127, a subrange never outside its bounds.
/// The constant may stand on either side.
#[rstest]
fn comparison_decided_by_the_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE Level : INT (0..10); END_TYPE

FUNCTION test : BOOL
VAR
    u : UINT;
    w : UDINT;
    s : SINT;
    l : Level;
END_VAR
    test := u < 0;
    test := w >= 0;
    IF 0 > u THEN
        test := s > 127;
    END_IF;
    test := l = 11;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"
    [L0103] Warning: constant condition
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     test := u < 0;
        |             ^^|^^
        |               `---- the comparison is always FALSE
        |
        | Note 1: UINT holds 0 to 65535
        |
        | Note 2: lint rule: constant-condition
    ----'
    [L0103] Warning: constant condition
        ,-[ file:///test0.st:12:13 ]
        |
     12 |     test := w >= 0;
        |             ^^^|^^
        |                `---- the comparison is always TRUE
        |
        | Note 1: UDINT holds 0 to 4294967295
        |
        | Note 2: lint rule: constant-condition
    ----'
    [L0103] Warning: constant condition
        ,-[ file:///test0.st:13:8 ]
        |
     13 |     IF 0 > u THEN
        |        ^^|^^
        |          `---- the comparison is always FALSE
        |
        | Note 1: UINT holds 0 to 65535
        |
        | Note 2: lint rule: constant-condition
    ----'
    [L0103] Warning: constant condition
        ,-[ file:///test0.st:14:17 ]
        |
     14 |         test := s > 127;
        |                 ^^^|^^^
        |                    `----- the comparison is always FALSE
        |
        | Note 1: SINT holds -128 to 127
        |
        | Note 2: lint rule: constant-condition
    ----'
    [L0103] Warning: constant condition
        ,-[ file:///test0.st:16:13 ]
        |
     16 |     test := l = 11;
        |             ^^^|^^
        |                `---- the comparison is always FALSE
        |
        | Note 1: Level holds 0 to 10
        |
        | Note 2: lint rule: constant-condition
    ----'
    ");
}

/// A comparison the type does not decide stays quiet: a value in range, a
/// signed integer against zero, a radix literal read as the INT it is, and
/// two constants, which a CONSTANT is there to switch.
#[rstest]
fn comparison_not_decided_by_the_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR CONSTANT
    DEBUG : INT := 3;
END_VAR
VAR
    u : UINT;
    i : INT;
    s : SINT;
    r : REAL;
END_VAR
    test := u > 0;
    test := u <= 100;
    test := i < 0;
    test := i = 16#FFFF;
    test := s >= -100 AND s < 100;
    test := DEBUG > 2;
    test := r < 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-condition"), @r"");
}
