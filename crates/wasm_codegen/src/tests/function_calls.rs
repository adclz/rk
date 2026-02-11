//! Function call code generation tests.

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_simple_function_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test_call : INT
        VAR
            result : INT;
        END_VAR
            result := add(1, 2);
            test_call := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_function_call_with_params(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION multiply : INT
        VAR_INPUT
            x : INT;
            y : INT;
        END_VAR
            multiply := x * y;
        END_FUNCTION

        FUNCTION test_params : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := multiply(a, b);
            test_params := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_function_call_in_expression(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION double : INT
        VAR_INPUT
            x : INT;
        END_VAR
            double := x * 2;
        END_FUNCTION

        FUNCTION test_expr : INT
        VAR_INPUT
            a : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := double(a) + double(a + 1);
            test_expr := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_chained_function_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add_one : INT
        VAR_INPUT
            x : INT;
        END_VAR
            add_one := x + 1;
        END_FUNCTION

        FUNCTION double : INT
        VAR_INPUT
            x : INT;
        END_VAR
            double := x * 2;
        END_FUNCTION

        FUNCTION test_chained : INT
        VAR_INPUT
            a : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := double(add_one(a));
            test_chained := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_recursive_function(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION factorial : INT
        VAR_INPUT
            n : INT;
        END_VAR
            IF n <= 1 THEN
                factorial := 1;
            ELSE
                factorial := n * factorial(n - 1);
            END_IF;
        END_FUNCTION

        FUNCTION test_recursive : INT
        VAR
            result : INT;
        END_VAR
            result := factorial(5);
            test_recursive := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}
