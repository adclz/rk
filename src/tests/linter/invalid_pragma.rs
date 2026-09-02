use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn test_on_function_valid(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION my_test : BOOL
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"");
}

#[rstest]
fn once_on_function_valid(mut with_db: RootDatabase) {
    let source = r#"
{once}
FUNCTION setup : INT
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"");
}

#[rstest]
fn once_on_program_invalid(mut with_db: RootDatabase) {
    let source = r#"
{once}
PROGRAM main
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"
    [L0003] Warning: invalid pragma for this POU
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {once}
       | ^^^|^^
       |    `---- {once} is not valid on PROGRAM
       |
       | Note: lint rule: invalid-pragma
    ---'
    ");
}

#[rstest]
fn warn_on_any_pou_valid(mut with_db: RootDatabase) {
    let source = r#"
{warn = 'deprecated'}
FUNCTION fn1 : INT
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"");
}
