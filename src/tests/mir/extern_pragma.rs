use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

#[rstest]
fn extern_pragma_generates_import(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION my_sqrt : REAL
VAR_INPUT IN : REAL; END_VAR
    {extern 'math' 'sqrt.REAL' (params IN) (result my_sqrt)}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import math.sqrt.REAL(Real) -> Real");
}

#[rstest]
fn extern_pragma_no_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION __ASSERT_FAIL
    {extern 'assert' 'fail'}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import assert.fail()");
}

#[rstest]
fn extern_pragma_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ext_add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    {extern 'math' 'add' (params a b) (result ext_add)}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import math.add(Int, Int) -> Int");
}

#[rstest]
fn extern_pragma_result_only(mut with_db: RootDatabase) {
    // Extern with no params but a result (e.g., clock.now)
    let source = r#"
FUNCTION get_time : LINT
    {extern 'wasi:clocks/monotonic-clock' 'now' (result get_time)}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import wasi:clocks/monotonic-clock.now() -> LInt");
}
