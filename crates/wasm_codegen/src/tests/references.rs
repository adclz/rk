//! Reference and pointer code generation tests (VAR_IN_OUT parameters).

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_var_in_out_read(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION increment : INT
        VAR_IN_OUT
            value : INT;
        END_VAR
            increment := value;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Use the module's own memory
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    // Instantiate module (it has its own memory)
    let instance = super::instantiate_with_memory(&mut store, &module);

    // Get the memory and write value 42 at address 0
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory.write(&mut store, 0, &42i32.to_le_bytes()).unwrap();

    // Call function with pointer to address 0
    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "increment")
        .unwrap();

    let result = func.call(&mut store, 0).unwrap();
    assert_eq!(result, 42, "Should read value from VAR_IN_OUT parameter");
}

#[rstest]
fn test_var_in_out_write(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set_value : INT
        VAR_IN_OUT
            target : INT;
        END_VAR
            target := 99;
            set_value := 0;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    // Instantiate module
    let instance = super::instantiate_with_memory(&mut store, &module);

    // Get memory and initialize with 0
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory.write(&mut store, 0, &0i32.to_le_bytes()).unwrap();

    // Call function with pointer
    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "set_value")
        .unwrap();

    func.call(&mut store, 0).unwrap();

    // Read back the value
    let mut buffer = [0u8; 4];
    memory.read(&store, 0, &mut buffer).unwrap();
    let value = i32::from_le_bytes(buffer);

    assert_eq!(value, 99, "Should write to VAR_IN_OUT parameter");
}

#[rstest]
fn test_var_in_out_increment(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION increment : INT
        VAR_IN_OUT
            value : INT;
        END_VAR
            value := value + 1;
            increment := value;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    // Instantiate module
    let instance = super::instantiate_with_memory(&mut store, &module);

    // Get memory and initialize with 10
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory.write(&mut store, 0, &10i32.to_le_bytes()).unwrap();

    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "increment")
        .unwrap();

    let result = func.call(&mut store, 0).unwrap();

    // Read back
    let mut buffer = [0u8; 4];
    memory.read(&store, 0, &mut buffer).unwrap();
    let value = i32::from_le_bytes(buffer);

    assert_eq!(value, 11, "Should increment VAR_IN_OUT parameter");
    assert_eq!(result, 11, "Should return incremented value");
}

#[rstest]
fn test_var_in_out_real(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION double_value : REAL
        VAR_IN_OUT
            value : REAL;
        END_VAR
            value := value * 2.0;
            double_value := value;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    // Instantiate module
    let instance = super::instantiate_with_memory(&mut store, &module);

    // Get memory and initialize with 3.5
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory.write(&mut store, 0, &3.5f32.to_le_bytes()).unwrap();

    let func = instance
        .get_typed_func::<i32, f32>(&mut store, "double_value")
        .unwrap();

    let result = func.call(&mut store, 0).unwrap();

    // Read back
    let mut buffer = [0u8; 4];
    memory.read(&store, 0, &mut buffer).unwrap();
    let value = f32::from_le_bytes(buffer);

    assert_eq!(value, 7.0, "Should double VAR_IN_OUT REAL parameter");
    assert_eq!(result, 7.0, "Should return doubled value");
}

#[rstest]
fn test_var_in_out_with_var_input(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add_to_ref : INT
        VAR_INPUT
            increment : INT;
        END_VAR
        VAR_IN_OUT
            target : INT;
        END_VAR
            target := target + increment;
            add_to_ref := target;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    // Instantiate module
    let instance = super::instantiate_with_memory(&mut store, &module);

    // Get memory and initialize target with 100
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory.write(&mut store, 0, &100i32.to_le_bytes()).unwrap();

    let func = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "add_to_ref")
        .unwrap();

    // Call with increment=25, pointer to target
    let result = func.call(&mut store, (25, 0)).unwrap();

    // Read back
    let mut buffer = [0u8; 4];
    memory.read(&store, 0, &mut buffer).unwrap();
    let value = i32::from_le_bytes(buffer);

    assert_eq!(value, 125, "Should add increment to VAR_IN_OUT target");
    assert_eq!(result, 125, "Should return updated value");
}
