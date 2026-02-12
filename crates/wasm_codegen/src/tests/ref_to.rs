//! REF_TO (pointer/reference type) code generation tests.

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_null_ref(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_null : REF_TO INT
            test_null := NULL;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    // Call function
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_null")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 0, "NULL should be represented as 0");
}

#[rstest]
fn test_ref_to_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION get_ptr_value : INT
        VAR_IN_OUT
            ptr : INT;
        END_VAR
        VAR
            ref_ptr : REF_TO INT;
        END_VAR
            ref_ptr := REF(ptr);
            get_ptr_value := ref_ptr^;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    // Get memory and set value at address 0
    let memory = instance.get_memory(&mut store, "memory").expect("Failed to get memory");
    memory.write(&mut store, 0, &42i32.to_le_bytes()).unwrap();

    // Call function with pointer to address 0
    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "get_ptr_value")
        .unwrap();

    let result = func.call(&mut store, 0).unwrap();
    assert_eq!(result, 42, "Should read value through REF_TO pointer");
}

#[rstest]
fn test_ref_to_assignment_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set_ptr_value : INT
        VAR_IN_OUT
            target : INT;
        END_VAR
        VAR
            ptr : REF_TO INT;
        END_VAR
            ptr := REF(target);
            ptr^ := 99;
            set_ptr_value := 0;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    // Get memory and initialize
    let memory = instance.get_memory(&mut store, "memory").expect("Failed to get memory");
    memory.write(&mut store, 0, &0i32.to_le_bytes()).unwrap();

    // Call function
    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "set_ptr_value")
        .unwrap();

    func.call(&mut store, 0).unwrap();

    // Read back the modified value
    let mut buffer = [0u8; 4];
    memory.read(&store, 0, &mut buffer).unwrap();
    let value = i32::from_le_bytes(buffer);

    assert_eq!(value, 99, "Should write through REF_TO pointer");
}

#[rstest]
fn test_ref_to_local_read(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_ref : INT
        VAR
            x : INT := 42;
            ptr : REF_TO INT;
        END_VAR
            ptr := REF(x);
            test_ref := ptr^;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_ref")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 42, "Should read value through pointer");
}

#[rstest]
fn test_ref_to_local_write(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_ref_assign : INT
        VAR
            x : INT := 10;
            ptr : REF_TO INT;
        END_VAR
            ptr := REF(x);
            ptr^ := 99;
            test_ref_assign := x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_ref_assign")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 99, "Should modify x through pointer");
}

#[rstest]
fn test_multiple_deref(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_double_deref : INT
        VAR
            x : INT := 5;
            ptr : REF_TO INT;
            ptrptr : REF_TO REF_TO INT;
        END_VAR
            ptr := REF(x);
            ptrptr := REF(ptr);
            test_double_deref := ptrptr^^;
        END_FUNCTION
        
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_double_deref")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 5, "Should dereference twice to get original value");
}
