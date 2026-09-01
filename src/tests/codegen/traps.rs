//! Runtime checks that fault the scan instead of computing a wrong answer:
//! the subrange range check (`rk.range_check_*`, the runtime half of E0802)
//! and the VM's own division traps. The array bounds check has its own tests
//! in `arrays.rs`.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// Run `run : DINT`, expect the call itself to fail, and return the fault
/// text a user would see — the decoded `$rk_exception` payload, or the
/// trap's own words. A subrange fault and a division trap must stay
/// distinguishable, or the diagnostic value collapses.
fn expect_fault(with_db: &mut db::RootDatabase, source: &str, why: &str) -> String {
    let wasm = compile_to_wasm(with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = f.call(&mut store, ()).expect_err(why);
    super::fault_message(&mut store, memory, err)
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
///
/// The value arrives already unsigned because it cannot arrive any other way:
/// `w := n` for a DINT `n` is E0301, so no signed value reaches an unsigned
/// subrange in the first place.
#[rstest]
fn a_negative_bit_pattern_faults_on_the_unsigned_lane(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : UDINT (0..4000000000); END_TYPE

        FUNCTION run : DINT
        VAR
            w : Wide;
            n : UDINT;
        END_VAR
            n := 4294967295;   (* -1's bit pattern, read unsigned *)
            w := n;
            run := 1;
        END_FUNCTION
    "#;
    expect_fault(
        &mut with_db,
        source,
        "4294967295 is above the upper bound of UDINT (0..4000000000)",
    );
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
    let msg = expect_fault(&mut with_db, source, "99 into an element of INT (0..10)");
    assert!(
        msg.contains("value out of subrange bounds"),
        "a range fault names the check: {msg}"
    );
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

/// The two checks COMPOSE on one store: `a[i] := n` with `i` a runtime
/// variable routes the address through `rk.idx_check` and the value through
/// `rk.range_check_*`. In-bounds index + out-of-range value must still fault
/// on the VALUE — the constant-index test alone would let the variable-index
/// address path skip the subrange wrap unnoticed.
#[rstest]
fn a_variable_index_store_still_checks_the_subrange(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            a : ARRAY[0..2] OF Small;
            i : INT;
            n : INT;
        END_VAR
            i := 1;
            n := 99;
            a[i] := n;
            run := a[i];
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "99 through a checked index");
    assert!(
        msg.contains("value out of subrange bounds"),
        "the VALUE check fires, not the (satisfied) index check: {msg}"
    );
}

/// Both violated at once: out-of-bounds index AND out-of-range value. ONE of
/// them must fault — pinned (not promised) to the index, since the address
/// is computed before the value converts.
#[rstest]
fn both_checks_violated_faults_on_one(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            a : ARRAY[0..2] OF Small;
            i : INT;
            n : INT;
        END_VAR
            i := 7;
            n := 99;
            a[i] := n;
            run := 0;
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "both violations must not cancel out");
    assert!(
        msg.contains("array index out of bounds"),
        "the INDEX check speaks first: {msg}"
    );
}

/// A subrange COUNTER is still a subrange. The initial store is checked
/// even when the body never runs — a zero-iteration loop still stores it.
#[rstest]
fn a_for_init_outside_the_subrange_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            i : Small;
            from : INT;
        END_VAR
            from := 99;   (* through a variable: a literal here is E0802 *)
            FOR i := from TO 0 DO
                run := run + 1;
            END_FOR;
            run := 0;
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "99 stored into the counter before any test");
    assert!(
        msg.contains("value out of subrange bounds"),
        "the init store names the check: {msg}"
    );
}

/// An ITERATE the body observes is checked: stepping 0,7,14 on a (0..10)
/// counter faults when 14 arrives, not before.
#[rstest]
fn a_for_iterate_leaving_the_subrange_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            i : Small;
        END_VAR
            FOR i := 0 TO 20 BY 7 DO
                run := run + i;
            END_FOR;
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "the third iterate is 14");
    assert!(
        msg.contains("value out of subrange bounds"),
        "the iterate names the check: {msg}"
    );
}

/// The DECLARED choice: the RANGE may overshoot the subrange as long as the
/// observed values do not. Stepping 0,7 on `TO 12` never reaches 14 — the
/// loop is legal and completes; faulting it would reject a correct program
/// on a bound it never touches.
#[rstest]
fn a_for_range_may_overshoot_when_the_values_do_not(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            i : Small;
        END_VAR
            run := 0;
            FOR i := 0 TO 12 BY 7 DO
                run := run + i;
            END_FOR;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 7, "iterates 0 and 7 run; 14 is never observed");
}

/// The full declared range walks its own subrange to the boundary.
#[rstest]
fn a_for_over_the_whole_subrange_is_clean(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION run : DINT
        VAR
            i : Small;
        END_VAR
            run := 0;
            FOR i := 0 TO 10 DO
                run := run + i;
            END_FOR;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 55, "0..=10 sums to 55 without a fault");
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

/// A null dereference FAULTS instead of accessing address 0.
///
/// Unchecked, this was not a fault at all: a read answered 0 and a write
/// silently succeeded. E1003 is the compile-time counterpart, and it does not
/// track a reference arriving as a parameter — which is how a null reaches a
/// callee in the first place.
#[rstest]
fn a_null_dereference_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION peek : INT
        VAR_INPUT p : REF_TO INT; END_VAR
            peek := p^;
        END_FUNCTION

        FUNCTION run : DINT
        VAR n : REF_TO INT := NULL; END_VAR
            run := peek(p := n);
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "a null read must fault");
    assert!(
        msg.contains("dereference of a null reference"),
        "unexpected fault message: {msg}"
    );
}

/// The write direction too: silently succeeding is what let a null with an
/// offset (`p^[i]`, `p^.field`) reach past the reserved floor and corrupt
/// live IEC variables.
#[rstest]
fn a_null_dereference_write_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION poke : INT
        VAR_INPUT p : REF_TO INT; END_VAR
            p^ := 1;
            poke := 0;
        END_FUNCTION

        FUNCTION run : DINT
        VAR n : REF_TO INT := NULL; END_VAR
            run := poke(p := n);
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "a null write must fault");
    assert!(
        msg.contains("dereference of a null reference"),
        "unexpected fault message: {msg}"
    );
}

/// An aggregate access through a null base faults at the dereference, before
/// the offset is added. `rk.idx_check` cannot catch this: it validates the
/// INDEX against the declared bounds and never sees the base, so an in-bounds
/// subscript on a null base used to compute a wild address and write to it.
#[rstest]
fn a_null_base_with_an_in_bounds_index_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Buf : ARRAY[0..99] OF INT; END_TYPE

        FUNCTION poke : INT
        VAR_INPUT p : REF_TO Buf; END_VAR
            p^[50] := 1;
            poke := 0;
        END_FUNCTION

        FUNCTION run : DINT
        VAR n : REF_TO Buf; END_VAR
            run := poke(p := n);
        END_FUNCTION
    "#;
    let msg = expect_fault(&mut with_db, source, "a null base must fault");
    assert!(
        msg.contains("dereference of a null reference"),
        "unexpected fault message: {msg}"
    );
}

/// A non-null dereference is untouched by the check.
#[rstest]
fn a_valid_dereference_does_not_fault(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION peek : INT
        VAR_INPUT p : REF_TO INT; END_VAR
            peek := p^;
        END_FUNCTION

        FUNCTION run : DINT
        VAR x : INT := 7; q : REF_TO INT; END_VAR
            q := REF(x);
            run := peek(p := q);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = crate::tests::codegen::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 7);
}
