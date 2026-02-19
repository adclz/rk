//! WASM execution tests (actually running the generated code).

use crate::tests::{compile_to_wasm, with_db};
use rstest::*;
use wasmtime::{Engine, Instance, Module, Store};

#[rstest]
fn test_execute_simple_arithmetic(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let add_func = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "add")
        .expect("Failed to get function");

    let result = add_func
        .call(&mut store, (5, 3))
        .expect("Failed to call function");
    assert_eq!(result, 8, "5 + 3 should equal 8");
}

#[rstest]
fn test_execute_factorial(mut with_db: db::RootDatabase) {
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
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let factorial_func = instance
        .get_typed_func::<i32, i32>(&mut store, "factorial")
        .expect("Failed to get function");

    let result = factorial_func
        .call(&mut store, 5)
        .expect("Failed to call function");
    assert_eq!(result, 120, "5! should equal 120");
}

#[rstest]
fn test_execute_chained_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION double : INT
        VAR_INPUT
            x : INT;
        END_VAR
            double := x * 2;
        END_FUNCTION

        FUNCTION add_one : INT
        VAR_INPUT
            x : INT;
        END_VAR
            add_one := x + 1;
        END_FUNCTION

        FUNCTION process : INT
        VAR_INPUT
            x : INT;
        END_VAR
            process := double(add_one(x));
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let process_func = instance
        .get_typed_func::<i32, i32>(&mut store, "process")
        .expect("Failed to get function");

    let result = process_func
        .call(&mut store, 5)
        .expect("Failed to call function");
    assert_eq!(result, 12, "double(add_one(5)) = double(6) = 12");
}

#[rstest]
fn test_execute_implicit_cast_int_to_real(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION int_to_real : REAL
        VAR_INPUT
            x : INT;
        END_VAR
        VAR
            result : REAL;
        END_VAR
            result := x;  // Implicit cast INT -> REAL
            int_to_real := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let func = instance
        .get_typed_func::<i32, f32>(&mut store, "int_to_real")
        .expect("Failed to get function");

    let result = func.call(&mut store, 42).expect("Failed to call function");
    assert_eq!(result, 42.0, "INT 42 should cast to REAL 42.0");
}

#[rstest]
fn test_execute_implicit_cast_in_arithmetic(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION mixed_arithmetic : REAL
        VAR_INPUT
            x : INT;
        END_VAR
        VAR
            a : REAL;
            result : REAL;
        END_VAR
            a := 2.5;
            result := a * x;  // Implicit cast x from INT -> REAL
            mixed_arithmetic := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let func = instance
        .get_typed_func::<i32, f32>(&mut store, "mixed_arithmetic")
        .expect("Failed to get function");

    let result = func.call(&mut store, 4).expect("Failed to call function");
    assert_eq!(result, 10.0, "2.5 * 4 should equal 10.0");
}
