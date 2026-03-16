use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

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
    {extern 'math' 'sqrt' (param IN) (result SQRT)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_extern_pragma_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    {extern 'math' 'add' (param a b) (result add)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_extern_pragma_unknown_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (param unknown_var) (result test)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0230] Error: extern variable not found
       ,-[ file:///test0.st:4:33 ]
       |
     4 |     {extern 'math' 'abs' (param unknown_var) (result test)}
       |                                 ^^^^^|^^^^^
       |                                      `------- no variable 'unknown_var' found in scope for extern pragma
    ---'
    ");
}

#[rstest]
fn invalid_extern_pragma_unknown_result(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (param x) (result bad_name)}
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0230] Error: extern variable not found
       ,-[ file:///test0.st:4:44 ]
       |
     4 |     {extern 'math' 'abs' (param x) (result bad_name)}
       |                                            ^^^^|^^^
       |                                                `----- no variable 'bad_name' found in scope for extern pragma
    ---'
    ");
}
