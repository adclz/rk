// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [L0003] Warning: pragma without effect on the POU
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {once}
       | ^^^|^^
       |    `---- {once} has no effect on a PROGRAM
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

/// `{must_call}` asks every instance to be called: on anything but a
/// FUNCTION_BLOCK it has no effect.
#[rstest]
fn must_call_outside_a_function_block(mut with_db: RootDatabase) {
    let source = r#"
{must_call}
FUNCTION f : INT
    f := 1;
END_FUNCTION

{must_call}
FUNCTION_BLOCK Fb
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "invalid-pragma"), @r"
    [L0003] Warning: pragma without effect on the POU
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {must_call}
       | ^^^^^|^^^^^
       |      `------- {must_call} has no effect on a FUNCTION
       |
       | Note: lint rule: invalid-pragma
    ---'
    ");
}
