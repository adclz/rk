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
fn extern_pragma_any_monomorphizes(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ABS : ANY_NUM
VAR_INPUT IN : INTO(ABS); END_VAR
    {extern 'math' 'abs' (params IN) (result ABS)}
END_FUNCTION

FUNCTION test : INT
VAR x : INT; y : REAL; END_VAR
    x := ABS(IN := -1);
    y := ABS(IN := -1.5);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test() -> Int
    import math.abs.INT(Int) -> Int [from ABS]
    import math.abs.REAL(Real) -> Real [from ABS]
    ");
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
fn extern_pragma_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Math
    FUNCTION ABS : ANY_NUM
    VAR_INPUT IN : INTO(ABS); END_VAR
        {extern 'math' 'abs' (params IN) (result ABS)}
    END_FUNCTION

    NAMESPACE Test
        USING Std.Math;

        {test}
        FUNCTION test_abs : INT
            test_abs := ABS(IN := -42);
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Std.Math.Test.test_abs() -> Int
    import math.abs.INT(Int) -> Int [from Std.Math.ABS]
    ");
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

#[rstest]
fn extern_from_multiple_files(mut with_db: RootDatabase) {
    let source1 = r#"
FUNCTION ABS : ANY_NUM
VAR_INPUT IN : INTO(ABS); END_VAR
    {extern 'math' 'abs' (params IN) (result ABS)}
END_FUNCTION
    "#;
    let source2 = r#"
FUNCTION test
VAR a : INT; END_VAR
    a := ABS(IN := -5);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source1, source2]), @r"
    export test()
    import math.abs.INT(Int) -> Int [from ABS]
    ");
}
