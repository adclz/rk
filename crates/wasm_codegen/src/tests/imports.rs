//! Tests for WASM import generation from extern pragmas.

use rstest::rstest;

use super::{compile_to_wasm, execute_wasm_with_imports, validate_wasm, with_db};

#[rstest]
fn test_extern_generates_valid_wasm(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (param x) (result my_abs)}
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    // Validation will fail because the import is not satisfied,
    // but the WASM binary itself should be structurally valid
    assert!(validate_wasm(&wasm_bytes).is_ok());
}

#[rstest]
fn test_extern_import_executes(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (param x) (result my_abs)}
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "my_abs", (-42i32,), |linker| {
        linker
            .func_wrap("math", "abs", |x: i32| -> i32 { x.abs() })
            .unwrap();
    });

    assert_eq!(result, 42);
}

#[rstest]
fn test_extern_with_real_type(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION my_sqrt : REAL
VAR_INPUT x : REAL; END_VAR
    {extern 'math' 'sqrt' (param x) (result my_sqrt)}
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result: f32 = execute_wasm_with_imports(&wasm_bytes, "my_sqrt", (9.0f32,), |linker| {
        linker
            .func_wrap("math", "sqrt", |x: f32| -> f32 { x.sqrt() })
            .unwrap();
    });

    assert!((result - 3.0).abs() < f32::EPSILON);
}

#[rstest]
fn test_extern_and_local_functions_coexist(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION ext_add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    {extern 'math' 'add' (param a b) (result ext_add)}
END_FUNCTION

FUNCTION double : INT
VAR_INPUT x : INT; END_VAR
    double := x + x;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute the local function (should work without providing imports for it)
    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "double", (21i32,), |linker| {
        linker
            .func_wrap("math", "add", |a: i32, b: i32| -> i32 { a + b })
            .unwrap();
    });

    assert_eq!(result, 42);

    // Execute the extern function
    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "ext_add", (10i32, 32i32), |linker| {
        linker
            .func_wrap("math", "add", |a: i32, b: i32| -> i32 { a + b })
            .unwrap();
    });

    assert_eq!(result, 42);
}

#[rstest]
fn test_local_function_calls_after_extern(mut with_db: db::RootDatabase) {
    // Ensure that local function indices are correct even when imports exist
    let source = r#"
FUNCTION ext_negate : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'negate' (param x) (result ext_negate)}
END_FUNCTION

FUNCTION add_one : INT
VAR_INPUT x : INT; END_VAR
    add_one := x + 1;
END_FUNCTION

FUNCTION add_two : INT
VAR_INPUT x : INT; END_VAR
    add_two := x + 2;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result: i32 =
        execute_wasm_with_imports(&wasm_bytes, "add_one", (10i32,), |linker| {
            linker
                .func_wrap("math", "negate", |x: i32| -> i32 { -x })
                .unwrap();
        });
    assert_eq!(result, 11);

    let result: i32 =
        execute_wasm_with_imports(&wasm_bytes, "add_two", (10i32,), |linker| {
            linker
                .func_wrap("math", "negate", |x: i32| -> i32 { -x })
                .unwrap();
        });
    assert_eq!(result, 12);
}
