//! Runtime checks that fault the scan instead of computing a wrong answer:
//! the subrange range check (`rk.range_check_*`, the runtime half of E0802)
//! and the VM's own division traps. The array bounds check has its own tests
//! in `arrays.rs`.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// Run `run : DINT`, expect the call itself to fail, and return the fault
/// text so a test can pin WHICH check spoke. A subrange fault and a division
/// trap must stay distinguishable, or the diagnostic value collapses.
fn expect_fault(with_db: &mut db::RootDatabase, source: &str, why: &str) -> String {
    let wasm = compile_to_wasm(with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = f.call(&mut store, ()).expect_err(why);
    format!("{err:?}")
}

/// An out-of-range value entering a subrange variable is DENIED at runtime:
/// the store raises an IEC exception and the scan faults, instead of storing
/// a value the type forbids. E0802 catches the constants; this catches what
/// only the running program knows.
#[rstest]
fn an_out_of_range_subrange_assignment_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        PROGRAM P
        VAR
            n : INT;
            s : Small;
        END_VAR
            n := n + 6;
            s := n;   (* 6, 12: the second scan leaves the range *)
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = runtime::Plc::load(&wasm, runtime::Config::default()).expect("load");
    plc.run(1).expect("6 is in range");
    let err = plc.scan().expect_err("12 leaves INT (0..10)");
    assert!(
        format!("{err:#}").contains("value out of subrange bounds"),
        "the fault names the check, got: {err:#}"
    );
}

/// Both bounds are IN range — the check must not be off by one, and a
/// negative lower bound is honoured.
#[rstest]
fn boundary_values_do_not_fault(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Band : INT (-5..10); END_TYPE

        FUNCTION run : DINT
        VAR
            s : Band;
            lo : INT;
            hi : INT;
        END_VAR
            lo := -5;
            hi := 10;
            s := lo;
            run := s;
            s := hi;
            run := run * 100 + s;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, -490, "-5 and 10 are both legal on INT (-5..10)");
}

/// The LOWER bound faults too — every other fault test overshoots high, and
/// below-lower is a different comparison.
#[rstest]
fn a_value_below_the_lower_bound_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            s : Small;
            n : INT;
        END_VAR
            n := -1;
            s := n;
            run := s;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "-1 is below INT (0..10)");
}

/// On the unsigned lane there is no "below zero": -1 as a UDINT bit pattern
/// is 4294967295, which the check refuses as ABOVE the upper bound. Either
/// way it faults, which is the contract; which comparison caught it is an
/// implementation detail.
#[rstest]
fn a_negative_bit_pattern_faults_on_the_unsigned_lane(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : UDINT (0..4000000000); END_TYPE

        FUNCTION run : DINT
        VAR
            w : Wide;
            n : DINT;
        END_VAR
            n := -1;
            w := n;
            run := 1;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "-1 reads as 4294967295 on the UDINT lane");
}

/// A subrange ARRAY ELEMENT checks its store like a plain variable.
#[rstest]
fn a_subrange_array_element_is_checked(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            a : ARRAY[0..2] OF Small;
            n : INT;
        END_VAR
            n := 99;
            a[1] := n;
            run := a[1];
        END_FUNCTION
    "#;
    // The message is pinned on the Plc path above — a bare wasmtime call
    // only sees "thrown Wasm exception"; the payload needs the runtime's
    // pending-exception decode.
    expect_fault(&mut with_db, source, "99 into an element of INT (0..10)");
}

/// A subrange STRUCT FIELD checks its store like a plain variable.
#[rstest]
fn a_subrange_struct_field_is_checked(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE
        TYPE Rec : STRUCT
            f : Small;
        END_STRUCT END_TYPE

        FUNCTION run : DINT
        VAR
            r : Rec;
            n : INT;
        END_VAR
            n := 99;
            r.f := n;
            run := r.f;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "99 into a field of INT (0..10)");
}

/// A subrange VAR_INPUT checks its argument at the call.
#[rstest]
fn a_subrange_function_input_is_checked(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION takes : DINT
        VAR_INPUT
            s : Small;
        END_VAR
            takes := s;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            n : INT;
        END_VAR
            n := 99;
            run := takes(n);
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "99 into a Small parameter");
}

/// A subrange FB INPUT checks the value written into the instance field.
#[rstest]
fn a_subrange_fb_input_is_checked(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION_BLOCK Holder
        VAR_INPUT
            s : Small;
        END_VAR
        VAR_OUTPUT
            o : INT;
        END_VAR
            o := s;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            h : Holder;
            n : INT;
        END_VAR
            n := 99;
            h(s := n);
            run := h.o;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "99 into a Small FB input");
}

/// A 64-bit base rides the i64 lane: the check has its own builtin there.
#[rstest]
fn a_64bit_subrange_is_checked(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Big : LINT (0..5000000000); END_TYPE

        FUNCTION run : DINT
        VAR
            b : Big;
            n : LINT;
        END_VAR
            n := LINT#6000000000;
            b := n;
            run := 1;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "6e9 leaves LINT (0..5e9)");
}

/// An unsigned base compares as its bit pattern: a UDINT bound above
/// `i32::MAX` is a negative i32, and only the unsigned builtin reads it
/// right. 3.9e9 is inside `(0..4e9)` — a signed compare would call it
/// negative and fault it.
#[rstest]
fn an_unsigned_subrange_accepts_above_i32_max(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : UDINT (0..4000000000); END_TYPE

        FUNCTION run : DINT
        VAR
            w : Wide;
            n : UDINT;
        END_VAR
            n := UDINT#3900000000;
            w := n;
            run := 1;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "3.9e9 is inside UDINT (0..4e9)");
}

#[rstest]
fn an_unsigned_subrange_still_faults_out_of_range(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : UDINT (0..4000000000); END_TYPE

        FUNCTION run : DINT
        VAR
            w : Wide;
            n : UDINT;
        END_VAR
            n := UDINT#4100000000;
            w := n;
            run := 1;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "4.1e9 leaves UDINT (0..4e9)");
}

/// Integer division by zero is the VM's own trap — no check of ours, but the
/// contract (a fault, not a wrong answer) is the same and deserves a pin.
#[rstest]
fn integer_division_by_zero_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : DINT;
            b : DINT;
        END_VAR
            a := 10;
            b := 0;
            run := a / b;
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "10 / 0 must fault");
    assert!(
        msg.contains("divide by zero") && !msg.contains("subrange"),
        "a division trap must not read like a range fault: {msg}"
    );
}

/// MOD by zero traps the same way.
#[rstest]
fn integer_modulo_by_zero_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : DINT;
            b : DINT;
        END_VAR
            a := 10;
            b := 0;
            run := a MOD b;
        END_FUNCTION
    "#;
    expect_fault(&mut with_db, source, "10 MOD 0 must fault");
}
