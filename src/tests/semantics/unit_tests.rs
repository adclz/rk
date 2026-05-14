use db::RootDatabase;
use hir::hir_ty::index_graphs::{discover_all_tests, find_test};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, test_diagnostics, with_db};

#[rstest]
fn discover_global_tests(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
END_FUNCTION

{test}
FUNCTION test_one : INT
END_FUNCTION

{test}
PROGRAM test_two
END_PROGRAM

PROGRAM normal
END_PROGRAM
"#;

    add_sources(&mut with_db, &[source]);
    let tests = discover_all_tests(&with_db);

    let names: Vec<_> = tests
        .iter()
        .map(|t| t.qualified_name().to_string())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"test_one".to_string()));
    assert!(names.contains(&"test_two".to_string()));
}

#[rstest]
fn discover_namespaced_tests(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Math

    {test}
    FUNCTION test_abs : INT
    END_FUNCTION

    FUNCTION ABS : INT
    END_FUNCTION

END_NAMESPACE

{test}
FUNCTION global_test : INT
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let tests = discover_all_tests(&with_db);

    let names: Vec<_> = tests
        .iter()
        .map(|t| t.qualified_name().to_string())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"Std.Math.test_abs".to_string()));
    assert!(names.contains(&"global_test".to_string()));
}

#[rstest]
fn find_test_by_qualified_name(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Math

    {test}
    FUNCTION test_sqrt : REAL
    END_FUNCTION

    FUNCTION SQRT : REAL
    END_FUNCTION

END_NAMESPACE

{test}
FUNCTION global_test : INT
END_FUNCTION

FUNCTION not_a_test : INT
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);

    // Find namespaced test
    assert!(find_test(&with_db, "Std.Math.test_sqrt").is_some());

    // Find global test
    assert!(find_test(&with_db, "global_test").is_some());

    // Non-test function should not be found
    assert!(find_test(&with_db, "not_a_test").is_none());

    // Non-existent function
    assert!(find_test(&with_db, "nonexistent").is_none());

    // Non-test namespaced function
    assert!(find_test(&with_db, "Std.Math.SQRT").is_none());
}

// --- {case} pragma validation ---

#[rstest]
fn valid_case_positional(mut with_db: RootDatabase) {
    let source = r#"
{test}
{case(5, 10)}
{case(-1, 1)}
FUNCTION test_fn : INT
VAR_INPUT x : INT; y : INT; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_named(mut with_db: RootDatabase) {
    let source = r#"
{test}
{case(x := 5, y := 10)}
FUNCTION test_fn : INT
VAR_INPUT x : INT; y : INT; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_case_too_many_args(mut with_db: RootDatabase) {
    let source = r#"
{test}
{case(5, 10, 99)}
FUNCTION test_fn : INT
VAR_INPUT x : INT; y : INT; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0206] Error: function call parameter mismatch
       ,-[ file:///test0.st:3:14 ]
       |
     3 | {case(5, 10, 99)}
       |              ^|
       |               `-- no parameter at index '2'
    ---'
    ");
}

#[rstest]
fn invalid_case_unknown_named_param(mut with_db: RootDatabase) {
    let source = r#"
{test}
{case(z := 5, y := 10)}
FUNCTION test_fn : INT
VAR_INPUT x : INT; y : INT; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0208] Error: function call parameter mismatch
       ,-[ file:///test0.st:3:7 ]
       |
     3 | {case(z := 5, y := 10)}
       |       |
       |       `-- unknown input parameter 'z'
    ---'
    ");
}
