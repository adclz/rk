use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn parens_around_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    test := (x);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unnecessary-parens"), @r"
    [L0311] Hint: unnecessary parentheses
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := (x);
       |             ^|^
       |              `--- unnecessary parentheses around 'x'
       |
       | Note: lint rule: unnecessary-parens
    ---'
    ");
}

#[rstest]
fn parens_around_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    test := (42);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unnecessary-parens"), @r"
    [L0311] Hint: unnecessary parentheses
       ,-[ file:///test0.st:3:13 ]
       |
     3 |     test := (42);
       |             ^^|^
       |               `--- unnecessary parentheses around '42'
       |
       | Note: lint rule: unnecessary-parens
    ---'
    ");
}

#[rstest]
fn parens_in_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : BOOL; END_VAR
    IF (x) THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unnecessary-parens"), @r"
    [L0311] Hint: unnecessary parentheses
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     IF (x) THEN
       |        ^|^
       |         `--- unnecessary parentheses around 'x'
       |
       | Note: lint rule: unnecessary-parens
    ---'
    ");
}

#[rstest]
fn parens_around_expression_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; y : INT; END_VAR
    test := (x + y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unnecessary-parens"), @r"");
}

#[rstest]
fn parens_around_comparison_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := (x > y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unnecessary-parens"), @r"");
}

#[rstest]
fn no_parens_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    test := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unnecessary-parens"), @r"");
}
