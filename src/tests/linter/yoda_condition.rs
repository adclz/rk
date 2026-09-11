use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn literal_on_left(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF 5 = x THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "yoda-condition"), @r"
    [L0313] Hint: yoda condition
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF 5 = x THEN
       |        ^^|^^
       |          `---- literal value on the left side of comparison
       |          |
       |          `---- swap both operands: 'x = 5'
       |
       | Note: lint rule: yoda-condition
    ---'
    ");
}

#[rstest]
fn literal_on_right(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x = 5 THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "yoda-condition"), @r"");
}

#[rstest]
fn both_literals(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    IF 5 = 5 THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "yoda-condition"), @r"");
}

#[rstest]
fn both_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
    y : INT;
END_VAR
    IF x = y THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "yoda-condition"), @r"");
}

#[rstest]
fn literal_lt(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF 0 < x THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "yoda-condition"), @r"
    [L0313] Hint: yoda condition
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF 0 < x THEN
       |        ^^|^^
       |          `---- literal value on the left side of comparison
       |          |
       |          `---- swap both operands: 'x < 0'
       |
       | Note: lint rule: yoda-condition
    ---'
    ");
}
