//! Function Block code generation tests.

use crate::tests::{compile_to_wasm, execute_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_simple_fb_method(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT := 0;
        END_VAR

        METHOD Increment : INT
            THIS.count := THIS.count + 1;
            Increment := THIS.count;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_fb_method_with_parameter(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Accumulator
        VAR
            total : INT := 0;
        END_VAR

        METHOD Add : INT
        VAR_INPUT
            value : INT;
        END_VAR
            THIS.total := THIS.total + value;
            Add := THIS.total;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_fb_multiple_instance_vars(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Calculator
        VAR
            result : INT;
            temp : INT;
        END_VAR

        METHOD Calculate : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            THIS.temp := a + b;
            THIS.result := THIS.temp * 2;
            Calculate := THIS.result;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_fb_method_execution(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR

        METHOD Increment : INT
            THIS.count := THIS.count + 1;
            Increment := THIS.count;
        END_METHOD

        METHOD GetCount : INT
            GetCount := THIS.count;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Create an FB instance in memory
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    // Get memory and allocate FB instance (just one INT: count = 0)
    let memory = instance.get_memory(&mut store, "memory").expect("Failed to get memory");
    let fb_address = 0i32; // FB instance at address 0
    memory.write(&mut store, fb_address as usize, &0i32.to_le_bytes()).unwrap();

    // Call Increment method (should increment count and return 1)
    let increment = instance
        .get_typed_func::<i32, i32>(&mut store, "Counter$Increment")
        .expect("Failed to get Increment method");

    let result1 = increment.call(&mut store, fb_address).unwrap();
    assert_eq!(result1, 1, "First increment should return 1");

    // Call Increment again (should return 2)
    let result2 = increment.call(&mut store, fb_address).unwrap();
    assert_eq!(result2, 2, "Second increment should return 2");

    // Call GetCount to verify state
    let get_count = instance
        .get_typed_func::<i32, i32>(&mut store, "Counter$GetCount")
        .expect("Failed to get GetCount method");

    let count = get_count.call(&mut store, fb_address).unwrap();
    assert_eq!(count, 2, "Count should be 2 after two increments");
}
