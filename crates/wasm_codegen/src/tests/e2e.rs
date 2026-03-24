//! End-to-end test: compile IEC → WASM → run with wasmtime.

use rstest::rstest;

use super::{compile_to_wasm, with_db};

#[rstest]
fn test_e2e_runtime(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs.INT' (params x) (result my_abs)}
END_FUNCTION

FUNCTION double : INT
VAR_INPUT x : INT; END_VAR
    double := x + x;
END_FUNCTION

FUNCTION add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    add := a + b;
END_FUNCTION

FUNCTION __ASSERT_FAIL
VAR_INPUT message : STRING; END_VAR
    {extern 'assert' 'fail' (params message)}
END_FUNCTION

FUNCTION assert_eq_int
VAR_INPUT value : INT; target : INT; END_VAR
    IF value <> target THEN
        __ASSERT_FAIL(message := '');
    END_IF;
END_FUNCTION

{test}
FUNCTION test_double
    assert_eq_int(value := double(x := 21), target := 42);
END_FUNCTION

{test}
FUNCTION test_add
    assert_eq_int(value := add(a := 10, b := 32), target := 42);
END_FUNCTION

{test}
FUNCTION test_abs
    assert_eq_int(value := my_abs(x := -7), target := 7);
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let failures = rk::test_runner::run_tests(&wasm_bytes, None);
    assert_eq!(failures, 0, "Expected all e2e tests to pass");
}
