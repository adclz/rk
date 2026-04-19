//! Struct execution tests - testing struct field access and manipulation.

use crate::tests::{compile_to_wasm, with_db};
use rstest::*;

#[rstest]
fn test_struct_field_access(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point :
        STRUCT
            x : INT;
            y : INT;
        END_STRUCT
        END_TYPE

        FUNCTION get_x : INT
        VAR
            p : Point;
        END_VAR
            p.x := 10;
            p.y := 20;
            get_x := p.x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let get_x = instance
        .get_typed_func::<(), i32>(&mut store, "get_x")
        .expect("Failed to get function");

    let result = get_x.call(&mut store, ()).unwrap();
    assert_eq!(result, 10, "Should read p.x value");
}

#[rstest]
fn test_struct_computation(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rectangle :
        STRUCT
            width : INT;
            height : INT;
        END_STRUCT
        END_TYPE

        FUNCTION area : INT
        VAR
            rect : Rectangle;
        END_VAR
            rect.width := 5;
            rect.height := 3;
            area := rect.width * rect.height;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let area = instance
        .get_typed_func::<(), i32>(&mut store, "area")
        .expect("Failed to get function");

    let result = area.call(&mut store, ()).unwrap();
    assert_eq!(result, 15, "5 * 3 = 15");
}

#[rstest]
fn test_nested_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point :
        STRUCT
            x : INT;
            y : INT;
        END_STRUCT
        END_TYPE

        TYPE Line :
        STRUCT
            start : Point;
            end_point : Point;
        END_STRUCT
        END_TYPE

        FUNCTION test_nested : INT
        VAR
            line : Line;
        END_VAR
            line.start.x := 1;
            line.start.y := 2;
            line.end_point.x := 10;
            line.end_point.y := 20;

            test_nested := line.end_point.x + line.start.x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let test_nested = instance
        .get_typed_func::<(), i32>(&mut store, "test_nested")
        .expect("Failed to get function");

    let result = test_nested.call(&mut store, ()).unwrap();
    assert_eq!(result, 11, "10 + 1 = 11");
}
