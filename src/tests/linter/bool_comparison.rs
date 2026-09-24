use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn eq_true(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x = TRUE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"
    [L0209] Info: comparison with boolean literal
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x = TRUE;
       |             ^^^^|^^^
       |                 `----- comparison with boolean literal can be simplified to the variable itself
       |
       | Note: lint rule: bool-comparison
    ---'
    ");
}

#[rstest]
fn eq_false(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x = FALSE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"
    [L0209] Info: comparison with boolean literal
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x = FALSE;
       |             ^^^^|^^^^
       |                 `------ comparison with boolean literal can be simplified to NOT variable
       |
       | Note: lint rule: bool-comparison
    ---'
    ");
}

#[rstest]
fn ne_true(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x <> TRUE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"
    [L0209] Info: comparison with boolean literal
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x <> TRUE;
       |             ^^^^|^^^^
       |                 `------ comparison with boolean literal can be simplified to NOT variable
       |
       | Note: lint rule: bool-comparison
    ---'
    ");
}

#[rstest]
fn ne_false(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := x <> FALSE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"
    [L0209] Info: comparison with boolean literal
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := x <> FALSE;
       |             ^^^^^|^^^^
       |                  `------ comparison with boolean literal can be simplified to the variable itself
       |
       | Note: lint rule: bool-comparison
    ---'
    ");
}

#[rstest]
fn if_condition_eq_true(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF flag = TRUE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"
    [L0209] Info: comparison with boolean literal
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF flag = TRUE THEN
       |        ^^^^^|^^^^^
       |             `------- comparison with boolean literal can be simplified to the variable itself
       |
       | Note: lint rule: bool-comparison
    ---'
    ");
}

#[rstest]
fn no_warning_bool_vs_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
    y : BOOL;
END_VAR
    test := x = y;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"");
}

#[rstest]
fn no_warning_int_comparison(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : INT;
END_VAR
    test := x = 5;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "bool-comparison"), @r"");
}

/// `BOOL#1` and `BOOL#0` are TRUE and FALSE, in any case. The rule compared
/// the literal's text with `TRUE`, so `x = BOOL#1` got the suggestion meant
/// for FALSE.
#[rstest]
#[case::one("x = BOOL#1", "the variable itself")]
#[case::zero("x = BOOL#0", "NOT variable")]
#[case::lower_case("x <> bool#true", "NOT variable")]
fn a_bool_literal_is_read_as_its_value(
    mut with_db: RootDatabase,
    #[case] comparison: &str,
    #[case] suggestion: &str,
) {
    let source = format!(
        r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := {comparison};
END_FUNCTION
"#
    );
    let rendered = test_single_lint(&mut with_db, &[&source], "bool-comparison");
    assert!(
        rendered.contains(&format!("can be simplified to {suggestion}")),
        "`{comparison}` must suggest {suggestion}, got:\n{rendered}"
    );
}
