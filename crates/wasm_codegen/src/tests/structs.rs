//! Tests for struct operations in WASM codegen.

use crate::tests::{compile_to_wasm, execute_wasm, with_db};
use rstest::*;

#[rstest]
fn test_simple_struct_read(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT
            x : INT;
            y : INT;
        END_STRUCT END_TYPE

        FUNCTION test_struct : INT
        VAR
            p : Point;
        END_VAR
            p.x := 10;
            p.y := 20;
            test_struct := p.x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify 
    let result: i32 = execute_wasm(&wasm_bytes, "test_struct", ());
    assert_eq!(result, 10);
}

#[rstest]
fn test_simple_struct_write(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT
            x : INT;
            y : INT;
        END_STRUCT END_TYPE

        FUNCTION test_struct : INT
        VAR
            p : Point;
        END_VAR
            p.x := 42;
            p.y := 100;
            test_struct := p.y;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify
    let result: i32 = execute_wasm(&wasm_bytes, "test_struct", ());
    assert_eq!(result, 100);
}

#[rstest]
fn test_struct_with_different_types(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE MixedData : STRUCT
            count : INT;
            value : REAL;
            flag : BOOL;
        END_STRUCT END_TYPE

        FUNCTION test_mixed : REAL
        VAR
            data : MixedData;
        END_VAR
            data.count := 5;
            data.value := 3.14;
            data.flag := TRUE;
            test_mixed := data.value;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify
    let result: f32 = execute_wasm(&wasm_bytes, "test_mixed", ());
    assert!((result - 3.14).abs() < 0.001);
}

#[rstest]
fn test_nested_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Inner : STRUCT
            a : INT;
            b : INT;
        END_STRUCT END_TYPE

        TYPE Outer : STRUCT
            inner : Inner;
            c : INT;
        END_STRUCT END_TYPE

        FUNCTION test_nested : INT
        VAR
            obj : Outer;
        END_VAR
            obj.inner.a := 10;
            obj.inner.b := 20;
            obj.c := 30;
            test_nested := obj.inner.a + obj.inner.b + obj.c;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify (10 + 20 + 30 = 60)
    let result: i32 = execute_wasm(&wasm_bytes, "test_nested", ());
    assert_eq!(result, 60);
}

#[rstest]
fn test_struct_field_calculation(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vector : STRUCT
            x : INT;
            y : INT;
        END_STRUCT END_TYPE

        FUNCTION magnitude_squared : INT
        VAR
            v : Vector;
        END_VAR
            v.x := 3;
            v.y := 4;
            magnitude_squared := v.x * v.x + v.y * v.y;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify (3^2 + 4^2 = 9 + 16 = 25)
    let result: i32 = execute_wasm(&wasm_bytes, "magnitude_squared", ());
    assert_eq!(result, 25);
}

#[rstest]
fn test_struct_in_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Counter : STRUCT
            value : INT;
            step : INT;
        END_STRUCT END_TYPE

        FUNCTION test_counter : INT
        VAR
            c : Counter;
            i : INT;
        END_VAR
            c.value := 0;
            c.step := 3;

            FOR i := 1 TO 5 DO
                c.value := c.value + c.step;
            END_FOR;

            test_counter := c.value;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify (5 iterations * 3 = 15)
    let result: i32 = execute_wasm(&wasm_bytes, "test_counter", ());
    assert_eq!(result, 15);
}

#[rstest]
fn test_struct_alignment(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE AlignTest : STRUCT
            a : INT;     (* 4 bytes, offset 0 *)
            b : BOOL;    (* 1 byte, offset 4 *)
            c : INT;     (* 4 bytes, offset 8 (padded) *)
        END_STRUCT END_TYPE

        FUNCTION test_alignment : INT
        VAR
            t : AlignTest;
        END_VAR
            t.a := 100;
            t.b := TRUE;
            t.c := 200;
            test_alignment := t.a + t.c;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify (100 + 200 = 300)
    let result: i32 = execute_wasm(&wasm_bytes, "test_alignment", ());
    assert_eq!(result, 300);
}

#[rstest]
fn test_struct_with_real_fields(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Temperature : STRUCT
            celsius : REAL;
            fahrenheit : REAL;
        END_STRUCT END_TYPE

        FUNCTION test_temp : REAL
        VAR
            temp : Temperature;
        END_VAR
            temp.celsius := 25.0;
            temp.fahrenheit := 77.0;
            test_temp := temp.celsius + temp.fahrenheit;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify (25.0 + 77.0 = 102.0)
    let result: f32 = execute_wasm(&wasm_bytes, "test_temp", ());
    assert!((result - 102.0).abs() < 0.001);
}
