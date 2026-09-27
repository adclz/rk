use db::RootDatabase;
use hir::hir_ty::index_graphs::{discover_all_tests, find_test};
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

#[rstest]
fn discover_global_tests(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
END_FUNCTION

{test}
FUNCTION test_one : INT
END_FUNCTION

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
    assert_eq!(names.len(), 1, "a PROGRAM is never a test (E1503)");
    assert!(names.contains(&"test_one".to_string()));
    assert!(!names.contains(&"test_two".to_string()));
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
