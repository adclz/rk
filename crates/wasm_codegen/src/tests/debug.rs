//! Tests for debug trap injection and instrumentation.

use crate::debug::{CodeGenConfig, DebugMode};
use crate::tests::{compile_to_wasm_with_config, with_db};
use rstest::*;

#[rstest]
fn test_debug_globals_exported(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT := 5;
        END_VAR
            test := x + 1;
        END_FUNCTION
    "#;

    let config = CodeGenConfig {
        debug_mode: DebugMode::StatementLevel,
        ..Default::default()
    };

    let (wasm_bytes, debug_info) = compile_to_wasm_with_config(&mut with_db, source, config);

    // Validate the WASM module
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).expect("WASM should be valid");

    // Check that debug globals are exported
    let exports: Vec<_> = module.exports().collect();
    let export_names: Vec<_> = exports.iter().map(|e| e.name()).collect();

    assert!(
        export_names.contains(&"debug_enabled"),
        "debug_enabled global should be exported. Exports: {:?}",
        export_names
    );
    assert!(
        export_names.contains(&"debug_trap_id"),
        "debug_trap_id global should be exported. Exports: {:?}",
        export_names
    );
}

#[rstest]
fn test_debug_mode_none_no_globals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
            test := 42;
        END_FUNCTION
    "#;

    let config = CodeGenConfig {
        debug_mode: DebugMode::None,
        ..Default::default()
    };

    let (wasm_bytes, _debug_info) = compile_to_wasm_with_config(&mut with_db, source, config);

    // Validate the WASM module
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).expect("WASM should be valid");

    // Check that debug globals are NOT exported
    let exports: Vec<_> = module.exports().collect();
    let export_names: Vec<_> = exports.iter().map(|e| e.name()).collect();

    assert!(
        !export_names.contains(&"debug_enabled"),
        "debug_enabled should not be exported in None mode. Exports: {:?}",
        export_names
    );
}

#[rstest]
fn test_debug_disabled_runs_normally(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION
    "#;

    let config = CodeGenConfig {
        debug_mode: DebugMode::StatementLevel,
        ..Default::default()
    };

    let (wasm_bytes, _debug_info) = compile_to_wasm_with_config(&mut with_db, source, config);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    // Get the debug_enabled global and ensure it's 0 (disabled)
    let debug_enabled = instance
        .get_global(&mut store, "debug_enabled")
        .expect("debug_enabled should exist");

    let val = debug_enabled.get(&mut store);
    assert_eq!(val.unwrap_i32(), 0, "debug_enabled should be 0 by default");

    // Run the function - should work normally with debug disabled
    let add_func = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "add")
        .expect("Failed to get function");

    let result = add_func
        .call(&mut store, (5, 3))
        .expect("Function should execute");
    assert_eq!(result, 8);
}

#[rstest]
#[should_panic(expected = "unreachable")]
fn test_debug_enabled_traps(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
            test := 42;
        END_FUNCTION
    "#;

    let config = CodeGenConfig {
        debug_mode: DebugMode::StatementLevel,
        ..Default::default()
    };

    let (wasm_bytes, debug_info) = compile_to_wasm_with_config(&mut with_db, source, config);

    println!("Debug info: {} trap(s) registered", debug_info.traps.len());

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    // Enable debugging by setting debug_enabled = 1
    let debug_enabled = instance
        .get_global(&mut store, "debug_enabled")
        .expect("debug_enabled should exist");
    debug_enabled
        .set(&mut store, wasmtime::Val::I32(1))
        .unwrap();

    // Try to run the function - should trap!
    let test_func = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .expect("Failed to get function");

    // This should panic with "unreachable"
    test_func.call(&mut store, ()).expect("Should trap");
}
