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
fn test_on_function_block_invalid(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION_BLOCK my_fb
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"
    [L0003] Warning: invalid pragma for this POU
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {test}
       | ^^^|^^
       |    `---- {test} is not valid on FUNCTION_BLOCK
       |
       | Note: lint rule: invalid-pragma
    ---'
    ");
}

#[rstest]
fn test_on_method_invalid(mut with_db: RootDatabase) {
    let source = r#"
CLASS c1
    {test}
    METHOD m1 : INT
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"
    [L0003] Warning: invalid pragma for this POU
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     {test}
       |     ^^^|^^
       |        `---- {test} is not valid on METHOD
       |
       | Note: lint rule: invalid-pragma
    ---'
    ");
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
