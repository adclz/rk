//! `MOD` executes with IEC's sign rule (the result takes the dividend's
//! sign), across widths. The semantics suite pins what is REJECTED; this
//! pins what the accepted shapes compute.

use rstest::rstest;

use super::{compile_to_wasm, with_db};

#[rstest]
fn modulo_of_ints(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            a : INT := 17;
            b : INT := 5;
        END_VAR
            test := a MOD b;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 2);
}

/// IEC: the result takes the DIVIDEND's sign — `-17 MOD 5 = -2`, not 3.
#[rstest]
fn modulo_takes_the_dividends_sign(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            a : INT := -17;
            b : INT := 5;
        END_VAR
            test := a MOD b;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, -2);
}

#[rstest]
fn modulo_of_lints(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LINT
        VAR
            a : LINT := 5000000003;
            b : LINT := 5;
        END_VAR
            test := a MOD b;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 3);
}

/// Unsigned MOD uses the unsigned remainder: `250 MOD 7` in the USINT domain.
#[rstest]
fn modulo_of_unsigned(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : USINT
        VAR
            a : USINT := 250;
            b : USINT := 7;
        END_VAR
            test := a MOD b;
        END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 5);
}
