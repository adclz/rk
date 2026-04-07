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
    [L0112] Warning: constant condition
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
    [L0112] Warning: constant condition
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
    [L0112] Warning: constant condition
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
    [L0112] Warning: constant condition
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
    [L0112] Warning: constant condition
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
    [L0132] Info: unnecessary parentheses
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     IF (TRUE) THEN
       |        ^^^|^^
       |           `---- unnecessary parentheses around 'TRUE'
       |
       | Note: lint rule: unnecessary-parens
    ---'
    [L0112] Warning: constant condition
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
    [L0112] Warning: constant condition
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     IF BOOL#TRUE THEN
       |        ^^^^|^^^^
       |            `------ IF condition is always TRUE
       |
       | Note: lint rule: constant-condition
    ---'
    [L0112] Warning: constant condition
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
