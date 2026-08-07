//! `**` executes: it lowers to the grafted `libm` pow, so these run the
//! whole path — graft, call resolution, operand casting — under wasmtime.
//! The semantics suite pins what is REJECTED; this pins what the accepted
//! shapes compute.

use rstest::rstest;

use super::{compile_to_wasm, with_db};

#[rstest]
fn power_of_real_literals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
            test := 2.0 ** 3.0;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 8.0);
}

/// The exponent may be any numeric: an INT variable is cast to the base.
#[rstest]
fn power_with_integer_exponent(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
        VAR
            x : REAL := 3.0;
            n : INT := 2;
        END_VAR
            test := x ** n;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 9.0);
}

/// A fractional exponent is a root — this is why `**` is a float pow and an
/// integer base is rejected.
#[rstest]
fn power_with_fractional_exponent(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
        VAR
            x : REAL := 9.0;
        END_VAR
            test := x ** 0.5;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 3.0);
}

#[rstest]
fn power_of_lreal(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LREAL
        VAR
            b : LREAL := 2.0;
        END_VAR
            test := b ** 10.0;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: f64 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 1024.0);
}
