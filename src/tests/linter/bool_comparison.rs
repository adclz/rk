use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0119] Advice: comparison with boolean literal
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0119] Advice: comparison with boolean literal
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0119] Advice: comparison with boolean literal
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0119] Advice: comparison with boolean literal
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0119] Advice: comparison with boolean literal
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}
