//! Basic function code generation tests.

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_empty_function_codegen(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR_INPUT
            x : INT;
        END_VAR
            test := x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_void_function(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION do_nothing
        VAR_INPUT
            x : INT;
        END_VAR
            // Empty body
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_function_with_local_variables(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION calc : INT
        VAR_INPUT
            x : INT;
        END_VAR
        VAR
            y : INT;
        END_VAR
            y := 10;
            calc := x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_function_with_multiple_inputs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_literals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_literals : INT
        VAR
            a : INT;
            b : BOOL;
            c : REAL;
        END_VAR
            a := 42;
            b := TRUE;
            c := 3.14;
            test_literals := a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_arithmetic_operations(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_arithmetic : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := a + b * 2 - 10 / 5;
            test_arithmetic := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_comparison_operations(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_comparison : BOOL
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR
            result : BOOL;
        END_VAR
            result := a > b;
            test_comparison := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_boolean_operations(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_boolean : BOOL
        VAR_INPUT
            a : BOOL;
            b : BOOL;
        END_VAR
        VAR
            result : BOOL;
        END_VAR
            result := a AND b OR NOT a;
            test_boolean := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_float_operations(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_float : REAL
        VAR_INPUT
            a : REAL;
            b : REAL;
        END_VAR
        VAR
            result : REAL;
        END_VAR
            result := a + b * 2.0 - 10.0 / 5.0;
            test_float := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}
