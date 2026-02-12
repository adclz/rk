//! Array execution tests - actually running WASM to verify array operations.

use crate::tests::{compile_to_wasm, with_db};
use rstest::*;

#[rstest]
fn test_array_write_and_read(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_array : INT
        VAR
            arr : ARRAY[0..4] OF INT;
        END_VAR
            arr[0] := 10;
            arr[1] := 20;
            arr[2] := 30;
            test_array := arr[1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let test_array = instance
        .get_typed_func::<(), i32>(&mut store, "test_array")
        .expect("Failed to get function");

    let result = test_array.call(&mut store, ()).unwrap();
    assert_eq!(result, 20, "Should read value 20 from arr[1]");
}

#[rstest]
fn test_array_sum(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_array : INT
        VAR
            arr : ARRAY[0..4] OF INT;
            sum : INT;
            i : INT;
        END_VAR
            arr[0] := 1;
            arr[1] := 2;
            arr[2] := 3;
            arr[3] := 4;
            arr[4] := 5;

            sum := 0;
            FOR i := 0 TO 4 DO
                sum := sum + arr[i];
            END_FOR;

            sum_array := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let sum_array = instance
        .get_typed_func::<(), i32>(&mut store, "sum_array")
        .expect("Failed to get function");

    let result = sum_array.call(&mut store, ()).unwrap();
    assert_eq!(result, 15, "Sum of 1+2+3+4+5 should be 15");
}

#[rstest]
fn test_2d_array_access(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_2d : INT
        VAR
            matrix : ARRAY[0..2, 0..2] OF INT;
        END_VAR
            matrix[0, 0] := 1;
            matrix[0, 1] := 2;
            matrix[1, 0] := 3;
            matrix[1, 1] := 4;

            test_2d := matrix[1, 1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let test_2d = instance
        .get_typed_func::<(), i32>(&mut store, "test_2d")
        .expect("Failed to get function");

    let result = test_2d.call(&mut store, ()).unwrap();
    assert_eq!(result, 4, "matrix[1,1] should be 4");
}

#[rstest]
fn test_array_with_non_zero_base(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_offset : INT
        VAR
            arr : ARRAY[10..14] OF INT;
        END_VAR
            arr[10] := 100;
            arr[11] := 200;
            arr[12] := 300;

            test_offset := arr[11];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let test_offset = instance
        .get_typed_func::<(), i32>(&mut store, "test_offset")
        .expect("Failed to get function");

    let result = test_offset.call(&mut store, ()).unwrap();
    assert_eq!(result, 200, "arr[11] should be 200");
}
