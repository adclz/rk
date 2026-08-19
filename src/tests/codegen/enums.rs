//! Enum codegen across contexts: literals resolve to declaration ordinals
//! (DInt storage), through FB fields, function params/returns, initializers,
//! CASE labels, arrays, and struct fields.
//! (Regression: `EnumValue` lowering was a stub emitting 0 for EVERY variant,
//! and `Type::EnumVariant` had no elementary mapping.)

use crate::tests::codegen::{compile_to_wasm_checked, with_db};
use rstest::*;

/// Enum VAR_INPUT written at the FB call site, compared against a literal in
/// the body, stored into enum instance state.
#[rstest]
fn fb_enum_input_and_state(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION_BLOCK picker
        VAR_INPUT c : Color; END_VAR
        VAR last : Color; END_VAR
        VAR_OUTPUT hit : INT; END_VAR
            IF c = Color#Green THEN
                hit := hit + 1;
            END_IF;
            last := c;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR p : picker; END_VAR
            p(c := Color#Red);
            p(c := Color#Green);
            p(c := Color#Green);
            test := p.hit;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 2, "two Green hits");
}

/// CASE over an enum: each variant label lowers to its own ordinal.
#[rstest]
fn case_over_enum_variants(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION pick : INT
        VAR_INPUT c : Color; END_VAR
            CASE c OF
                Color#Red: pick := 10;
                Color#Green: pick := 20;
                Color#Blue: pick := 30;
            END_CASE;
        END_FUNCTION

        FUNCTION test : INT
            test := pick(c := Color#Blue) * 100 + pick(c := Color#Green);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 3020, "Blue -> 30, Green -> 20");
}

/// Function-local enum with an initializer, reassignment, and equality /
/// inequality comparisons.
#[rstest]
fn function_local_enum(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle, Run, Halt); END_TYPE

        FUNCTION test : INT
        VAR m : Mode := Mode#Run; END_VAR
            test := 0;
            IF m = Mode#Run THEN
                test := test + 1;
            END_IF;
            m := Mode#Halt;
            IF m <> Mode#Run THEN
                test := test + 10;
            END_IF;
            IF m = Mode#Halt THEN
                test := test + 100;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 111, "init honored, reassign + =/<> comparisons");
}

/// Enum as a FUNCTION return value, consumed by the caller's comparison.
#[rstest]
fn enum_function_return(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle, Run, Halt); END_TYPE

        FUNCTION next : Mode
        VAR_INPUT m : Mode; END_VAR
            IF m = Mode#Idle THEN
                next := Mode#Run;
            ELSE
                next := Mode#Halt;
            END_IF;
        END_FUNCTION

        FUNCTION test : INT
        VAR m : Mode; END_VAR
            m := next(m := Mode#Idle);
            IF m = Mode#Run THEN
                test := 1;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "Idle -> Run round-trips through the return");
}

/// Arrays of enums: element writes and reads keep distinct ordinals.
#[rstest]
fn array_of_enums(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION test : INT
        VAR a : ARRAY[0..2] OF Color; i : INT; hits : INT; END_VAR
            a[0] := Color#Blue;
            a[1] := Color#Green;
            a[2] := Color#Blue;
            FOR i := 0 TO 2 DO
                IF a[i] = Color#Blue THEN
                    hits := hits + 1;
                END_IF;
            END_FOR;
            test := hits;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 2, "two Blue elements");
}

/// An enum field inside a struct: write via the field path, compare back.
#[rstest]
fn enum_struct_field(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE
        TYPE Pixel : STRUCT c : Color; brightness : INT; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR p : Pixel; END_VAR
            p.c := Color#Green;
            p.brightness := 7;
            IF p.c = Color#Green THEN
                test := p.brightness;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 7, "enum struct field round-trips");
}

/// IEC 61131-3: an enumerated type may assign explicit values to its
/// enumerators (`(Idle := 10, Run := 20)`), and every later enumerator without
/// one continues from the previous value. MIR used to number variants by
/// position and ignore `EnumVariant.value` entirely, so the declared values
/// were silently replaced by 0, 1, 2 — wrong for any CASE dispatch, INT
/// comparison, or value transmitted to a device.
#[rstest]
fn explicit_enum_values_are_honored(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle := 10, Run := 20, Halt := 30); END_TYPE

        FUNCTION code : DINT
        VAR_INPUT m : Mode; END_VAR
            CASE m OF
                Mode#Idle: code := 1;
                Mode#Run:  code := 2;
                Mode#Halt: code := 3;
            ELSE
                code := -1;
            END_CASE;
        END_FUNCTION

        FUNCTION test : DINT
            test := code(m := Mode#Run) * 100 + code(m := Mode#Halt);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 203,
        "CASE must match on the declared values (Run -> 2, Halt -> 3), not positions"
    );
}

/// Enumerators after an explicit value continue from it (`(A := 5, B, C)` is
/// 5, 6, 7) — the IEC/C-style continuation rule.
#[rstest]
fn enum_values_continue_after_an_explicit_one(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Step : (A := 5, B, C); END_TYPE

        FUNCTION code : DINT
        VAR_INPUT s : Step; END_VAR
            CASE s OF
                Step#A: code := 1;
                Step#B: code := 2;
                Step#C: code := 3;
            ELSE
                code := -1;
            END_CASE;
        END_FUNCTION

        FUNCTION test : DINT
            test := code(s := Step#B) * 10 + code(s := Step#C);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 23, "B and C continue from A's explicit 5 (6 and 7)");
}

/// The declared base is the storage: a SINT-based enum is ONE byte of
/// instance state, an LINT-based one is eight, and a variant value above
/// 2^31 survives. All three were wrong under the hardcoded DInt storage:
/// 4x-wide layout, truncated values, and an i32 literal lane meeting an
/// i64 load. The retain band is the observable: real layout, real bytes.
#[rstest]
fn typed_enum_storage_follows_the_declared_base(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : SINT (Lo, Hi); END_TYPE
        TYPE Wide : LINT (Zero, Big := 16#1_0000_0000); END_TYPE

        PROGRAM P
        VAR RETAIN s : Small; w : Wide; END_VAR
            s := Small#Hi;
            w := Wide#Big;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = super::compile_to_mir_and_wasm(&mut with_db, source);
    // s: 1 byte at 0, w: 8 bytes aligned up to offset 8 — 16 in all. The
    // hardcoded-DInt world packed both as 4-byte fields into 8.
    assert_eq!(mir.retain_size, 16, "storage widths must follow the bases");

    let mut plc = runtime::Plc::load(&wasm, runtime::Config::default()).expect("load");
    plc.run(1).expect("scan");
    let band = plc.read_retain();
    assert_eq!(band[0] as i8, 1, "Small#Hi is one SINT byte");
    assert_eq!(
        i64::from_le_bytes(band[8..16].try_into().unwrap()),
        0x1_0000_0000,
        "Wide#Big keeps its 33-bit value"
    );
}

/// Comparisons and CASE run at the declared lane — a wide enum's equality
/// is an i64 compare fed by an i64 literal, not a truncated i32 one.
#[rstest]
fn wide_enum_compares_and_matches_at_its_lane(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : LINT (Zero, Big := 16#1_0000_0000, Bigger); END_TYPE

        FUNCTION pick : DINT
        VAR_INPUT w : Wide; END_VAR
            CASE w OF
                Wide#Big:    pick := 1;
                Wide#Bigger: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION test : DINT
        VAR w : Wide; ok : DINT; END_VAR
            w := Wide#Bigger;
            IF w = Wide#Bigger THEN ok := 100; END_IF;
            test := ok + pick(w := Wide#Big) * 10 + pick(w := w);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 112, "equality 100 + Big 10 + Bigger 2");
}
