use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

#[rstest]
fn valid_extern_pragma_minimal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs'}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_extern_pragma_with_params_and_result(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SQRT : REAL
VAR_INPUT IN : REAL; END_VAR
    {extern 'math' 'sqrt' (params IN) (result SQRT)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_extern_pragma_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    {extern 'math' 'add' (params a b) (result add)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_extern_pragma_unknown_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (params unknown_var) (result test)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0230] Error: extern variable not found
       ,-[ file:///test0.st:4:34 ]
       |
     4 |     {extern 'math' 'abs' (params unknown_var) (result test)}
       |                                  ^^^^^|^^^^^
       |                                       `------- no variable 'unknown_var' found in scope for extern pragma
    ---'
    ");
}

#[rstest]
fn invalid_extern_pragma_unknown_result(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (params x) (result bad_name)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0230] Error: extern variable not found
       ,-[ file:///test0.st:4:45 ]
       |
     4 |     {extern 'math' 'abs' (params x) (result bad_name)}
       |                                             ^^^^|^^^
       |                                                 `----- no variable 'bad_name' found in scope for extern pragma
    ---'
    ");
}

// --- Test pragma visibility ---

#[rstest]
fn valid_test_pragma_referencing_normal_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_test_pragma_referencing_test_pou(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := test_helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_non_test_referencing_test_function(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_only : INT
END_FUNCTION

FUNCTION normal : INT
VAR x : INT; END_VAR
    x := test_only();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: access control violation
       ,-[ file:///test0.st:8:10 ]
       |
     8 |     x := test_only();
       |          ^^^^|^^^^
       |              `------ can not access test item 'test_only'
       |
       | Note: items marked with {test} can only be referenced from other {test} items
    ---'
    ");
}

#[rstest]
fn invalid_non_test_referencing_test_program(mut with_db: RootDatabase) {
    let source = r#"
{test}
PROGRAM test_prog
END_PROGRAM

FUNCTION normal : INT
VAR x : INT; END_VAR
END_FUNCTION"#;

    // Programs are not visible to functions anyway (only config scopes),
    // so no E0404 emitted here - just verifying no crash.
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
