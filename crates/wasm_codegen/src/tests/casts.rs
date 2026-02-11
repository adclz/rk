//! Implicit cast code generation tests.

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_implicit_cast_sint_to_int(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_cast : INT
        VAR
            a : SINT;
            b : INT;
        END_VAR
            a := 10;
            b := a;  // Implicit cast SINT -> INT
            test_cast := b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_implicit_cast_int_to_lint(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_cast : LINT
        VAR
            a : INT;
            b : LINT;
        END_VAR
            a := 100;
            b := a;  // Implicit cast INT -> LINT
            test_cast := b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_implicit_cast_int_to_real(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_cast : REAL
        VAR
            a : INT;
            b : REAL;
        END_VAR
            a := 42;
            b := a;  // Implicit cast INT -> REAL
            test_cast := b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_implicit_cast_real_to_lreal(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_cast : LREAL
        VAR
            a : REAL;
            b : LREAL;
        END_VAR
            a := 3.14;
            b := a;  // Implicit cast REAL -> LREAL
            test_cast := b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_implicit_cast_chained(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_cast : LREAL
        VAR
            a : SINT;
            b : INT;
            c : REAL;
            d : LREAL;
        END_VAR
            a := 10;
            b := a;   // SINT -> INT
            c := b;   // INT -> REAL
            d := c;   // REAL -> LREAL
            test_cast := d;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}
