//! FUNCTION overloads lower to distinct WASM symbols (via the signature
//! discriminant) and each call routes to the right one.

use crate::tests::codegen::{compile_to_wasm, compile_to_wasm_checked, with_db};
use rstest::*;

// Two `add` overloads (1-arg adds one, 2-arg sums) coexist as distinct wasm
// functions; `test` calls both and gets each result.
#[rstest]
fn overloaded_functions_execute_independently(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION

        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test : INT
            test := add(10) + add(3, 4);
        END_FUNCTION
    "#;

    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 18, "add/1(10) = 11, add/2(3, 4) = 7");
}

// Overloading works for non-integer types too — two REAL overloads, one per
// arity, each lowered and called independently.
#[rstest]
fn overloaded_real_functions(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION avg : REAL
        VAR_INPUT a : REAL; END_VAR
            avg := a;
        END_FUNCTION

        FUNCTION avg : REAL
        VAR_INPUT a : REAL; b : REAL; END_VAR
            avg := (a + b) / 2.0;
        END_FUNCTION

        FUNCTION test : REAL
            test := avg(3.0) + avg(2.0, 8.0);   // 3.0 + 5.0
        END_FUNCTION
    "#;

    let wasm = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 8.0, "avg/1(3.0) = 3.0, avg/2(2.0, 8.0) = 5.0");
}

// Same arity, different parameter TYPE — `conv(INT)` and `conv(REAL)` lower to
// distinct symbols (conv$Int / conv$Real) and each call routes by argument type.
#[rstest]
fn same_arity_typed_overloads(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION conv : INT
        VAR_INPUT a : INT; END_VAR
            conv := a * 2;
        END_FUNCTION

        FUNCTION conv : INT
        VAR_INPUT a : REAL; END_VAR
            conv := 99;
        END_FUNCTION

        FUNCTION test : INT
        VAR i : INT; END_VAR
            i := 5;
            test := conv(i) + conv(1.0);   // conv(INT)=10 + conv(REAL)=99
        END_FUNCTION
    "#;

    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 109, "conv(INT 5) = 10, conv(REAL 1.0) = 99");
}

/// The overload chosen for an indexed argument must be the one that actually
/// runs — a resolution bug here is silent at the call site and only shows up in
/// the value produced.
#[rstest]
fn indexed_argument_dispatches_to_the_right_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION tag : DINT
        VAR_INPUT v : INT; END_VAR
            tag := 1;
        END_FUNCTION

        FUNCTION tag : DINT
        VAR_INPUT v : DINT; END_VAR
            tag := 2;
        END_FUNCTION

        FUNCTION test : DINT
        VAR
            ints  : ARRAY[0..1] OF INT;
            dints : ARRAY[-1..1] OF DINT;
        END_VAR
            test := tag(ints[0]) * 10 + tag(dints[-1]);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 12, "INT element -> overload 1, DINT element -> overload 2");
}

/// RETURN-directed overloads emit as DISTINCT wasm functions and each call
/// routes to the overload its target picked. The params-only mangle put both
/// on one symbol, and module lowering silently skipped the second body — so
/// both assignments would have run the same function.
#[rstest]
fn return_overloads_route_by_target(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT
            G := 41;
        END_FUNCTION
        FUNCTION G : DINT
            G := 4200;
        END_FUNCTION
        FUNCTION test : DINT
        VAR i : INT; d : DINT; END_VAR
            i := G();
            d := G();
            test := d + INT_TO_DINT(i);
        END_FUNCTION
        FUNCTION INT_TO_DINT : DINT
        VAR_INPUT IN : INT; END_VAR
            {wasm 'nop' (params IN) (result INT_TO_DINT)}
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 4241, "each target got its own overload: 4200 + 41");
}

/// An untyped integer literal picks the INT overload by EXACT match; the REAL
/// candidate would also accept it by promotion, so this pins the rule rather
/// than letting either fall out.
#[rstest]
fn untyped_literal_prefers_exact_int(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION conv : INT
        VAR_INPUT a : INT; END_VAR
            conv := a * 2;
        END_FUNCTION
        FUNCTION conv : INT
        VAR_INPUT a : REAL; END_VAR
            conv := 99;
        END_FUNCTION
        FUNCTION test : INT
            test := conv(5);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 10, "5 is INT by exact match, not REAL by promotion");
}

/// The declared arity preference at runtime: add(10) runs add/1.
#[rstest]
fn full_arity_beats_defaulted_at_runtime(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT := 100; END_VAR
            add := a + b;
        END_FUNCTION
        FUNCTION test : INT
            test := add(10);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 11, "add/1 wins over add/2 padded with its default");
}

/// The INITIALIZER path lowers separately from statements — "type-checked to
/// the right overload" does not prove "emitted a call to the right symbol",
/// which is precisely how the params-only mangle bug hid.
#[rstest]
fn return_overload_in_initializer_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT
            G := 41;
        END_FUNCTION
        FUNCTION G : DINT
            G := 4200;
        END_FUNCTION
        FUNCTION test : DINT
        VAR d : DINT := G(); END_VAR
            test := d;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 4200, "the initializer called the DINT overload");
}

/// And the return-slot path: `test := G()` inside a DINT function.
#[rstest]
fn return_overload_in_return_slot_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT
            G := 41;
        END_FUNCTION
        FUNCTION G : DINT
            G := 4200;
        END_FUNCTION
        FUNCTION test : DINT
            test := G();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 4200, "the return slot picked and ran the DINT overload");
}

/// An overloaded call in an INITIALIZER: its resolution lives in init
/// inference, which the callee-symbol lookup never consulted — the call
/// lowered under its BARE name and died in codegen as an unknown function.
#[rstest]
fn overloaded_call_in_initializer_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION
        FUNCTION test : INT
        VAR x : INT := add(1, 2); END_VAR
            test := x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 3, "the initializer called the resolved 2-arg overload");
}
