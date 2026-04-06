use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn same_namespace_twice(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE MyProject
    FUNCTION fn1 : INT END_FUNCTION
END_NAMESPACE

NAMESPACE MyProject
    FUNCTION fn2 : INT END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0123] Advice: duplicate namespace in same file
       ,-[ file:///test0.st:6:11 ]
       |
     2 | NAMESPACE MyProject
       |           ^^^^|^^^^
       |               `------ first declaration of 'MyProject' here
       |
     6 | NAMESPACE MyProject
       |           ^^^^|^^^^
       |               `------ NAMESPACE 'MyProject' is declared multiple times in this file, consider merging
       |
       | Note: lint rule: duplicate-namespace
    ---'
    ");
}

#[rstest]
fn nested_same_namespace_twice(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE MyProject.Motors
    FUNCTION fn1 : INT END_FUNCTION
END_NAMESPACE

NAMESPACE MyProject.Motors
    FUNCTION fn2 : INT END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0123] Advice: duplicate namespace in same file
       ,-[ file:///test0.st:6:11 ]
       |
     2 | NAMESPACE MyProject.Motors
       |           ^^^^^^^^|^^^^^^^
       |                   `--------- first declaration of 'MyProject.Motors' here
       |
     6 | NAMESPACE MyProject.Motors
       |           ^^^^^^^^|^^^^^^^
       |                   `--------- NAMESPACE 'MyProject.Motors' is declared multiple times in this file, consider merging
       |
       | Note: lint rule: duplicate-namespace
    ---'
    ");
}

#[rstest]
fn different_namespaces_no_warning(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE MyProject.Motors
    FUNCTION fn1 : INT END_FUNCTION
END_NAMESPACE

NAMESPACE MyProject.Sensors
    FUNCTION fn2 : INT END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn three_times(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Ns
    FUNCTION fn1 : INT END_FUNCTION
END_NAMESPACE

NAMESPACE Ns
    FUNCTION fn2 : INT END_FUNCTION
END_NAMESPACE

NAMESPACE Ns
    FUNCTION fn3 : INT END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0123] Advice: duplicate namespace in same file
       ,-[ file:///test0.st:6:11 ]
       |
     2 | NAMESPACE Ns
       |           ^|
       |            `-- first declaration of 'Ns' here
       |
     6 | NAMESPACE Ns
       |           ^|
       |            `-- NAMESPACE 'Ns' is declared multiple times in this file, consider merging
       |
       | Note: lint rule: duplicate-namespace
    ---'
    [L0123] Advice: duplicate namespace in same file
        ,-[ file:///test0.st:10:11 ]
        |
      2 | NAMESPACE Ns
        |           ^|
        |            `-- first declaration of 'Ns' here
        |
     10 | NAMESPACE Ns
        |           ^|
        |            `-- NAMESPACE 'Ns' is declared multiple times in this file, consider merging
        |
        | Note: lint rule: duplicate-namespace
    ----'
    ");
}
