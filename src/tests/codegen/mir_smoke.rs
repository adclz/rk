//! Smoke tests for the MIR-based codegen pipeline.

use crate::tests::codegen::{compile_to_wasm, execute_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_mir_simple_add(mut with_db: db::RootDatabase) {
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
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result: i32 = execute_wasm(&wasm_bytes, "add", (5i32, 3i32));
    assert_eq!(result, 8);
}

#[rstest]
fn test_mir_if_else(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION max_val : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            IF a > b THEN
                max_val := a;
            ELSE
                max_val := b;
            END_IF;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result: i32 = execute_wasm(&wasm_bytes, "max_val", (10i32, 20i32));
    assert_eq!(result, 20);
}

#[rstest]
fn test_mir_for_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_to : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            i : INT;
        END_VAR
            sum_to := 0;
            FOR i := 1 TO n DO
                sum_to := sum_to + i;
            END_FOR;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result: i32 = execute_wasm(&wasm_bytes, "sum_to", 10i32);
    assert_eq!(result, 55);
}

#[rstest]
fn test_mir_case(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION classify : INT
        VAR_INPUT
            x : INT;
        END_VAR
            CASE x OF
                1: classify := 10;
                2: classify := 20;
            ELSE
                classify := 0;
            END_CASE;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result: i32 = execute_wasm(&wasm_bytes, "classify", 2i32);
    assert_eq!(result, 20);
}

/// A subrange bounded by a CONSTANT compiles and runs: the bounds fold
/// through the same evaluator as CASE labels and FOR steps. This shape used
/// to pass `rk check` and abort MIR with "expected a constant integer".
#[rstest]
fn subrange_constant_bound_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR CONSTANT K : INT := 5; END_VAR
        VAR x : INT (0..K); END_VAR
            x := 3;
            test := x;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 3, "the CONSTANT-bounded subrange lowered and executed");
}
