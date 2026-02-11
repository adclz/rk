//! Tests for array operations in WASM codegen.

use crate::tests::{compile_to_wasm, execute_wasm, with_db};
use rstest::*;

#[rstest]
fn test_simple_1d_array_read(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_array : INT
        VAR
            arr : ARRAY[0..2] OF INT;
        END_VAR
            arr[0] := 10;
            arr[1] := 20;
            arr[2] := 30;
            test_array := arr[1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify
    let result: i32 = execute_wasm(&wasm_bytes, "test_array", ());
    assert_eq!(result, 20);
}

#[rstest]
fn test_simple_1d_array_write(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_array : INT
        VAR
            arr : ARRAY[1..5] OF INT;
            i : INT;
        END_VAR
            i := 3;
            arr[i] := 42;
            test_array := arr[3];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify
    let result: i32 = execute_wasm(&wasm_bytes, "test_array", ());
    assert_eq!(result, 42);
}

#[rstest]
fn test_2d_array_access(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_matrix : INT
        VAR
            matrix : ARRAY[0..1, 0..2] OF INT;
        END_VAR
            matrix[0, 0] := 1;
            matrix[0, 1] := 2;
            matrix[0, 2] := 3;
            matrix[1, 0] := 4;
            matrix[1, 1] := 5;
            matrix[1, 2] := 6;
            test_matrix := matrix[1, 1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify (should return 5)
    let result: i32 = execute_wasm(&wasm_bytes, "test_matrix", ());
    assert_eq!(result, 5);
}

#[rstest]
fn test_array_of_reals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_real_array : REAL
        VAR
            arr : ARRAY[1..3] OF REAL;
        END_VAR
            arr[1] := 1.5;
            arr[2] := 2.5;
            arr[3] := 3.5;
            test_real_array := arr[2];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify
    let result: f32 = execute_wasm(&wasm_bytes, "test_real_array", ());
    assert_eq!(result, 2.5);
}

#[rstest]
fn test_array_in_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_array : INT
        VAR
            arr : ARRAY[0..4] OF INT;
            i : INT;
            sum : INT;
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

    // Execute and verify (1+2+3+4+5 = 15)
    let result: i32 = execute_wasm(&wasm_bytes, "sum_array", ());
    assert_eq!(result, 15);
}

#[rstest]
fn test_array_with_negative_indices(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_neg_indices : INT
        VAR
            arr : ARRAY[-2..2] OF INT;
        END_VAR
            arr[-2] := 10;
            arr[-1] := 20;
            arr[0] := 30;
            arr[1] := 40;
            arr[2] := 50;
            test_neg_indices := arr[-1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute and verify
    let result: i32 = execute_wasm(&wasm_bytes, "test_neg_indices", ());
    assert_eq!(result, 20);
}
