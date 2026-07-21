//! Enum codegen across contexts: literals resolve to declaration ordinals
//! (DInt storage), through FB fields, function params/returns, initializers,
//! CASE labels, arrays, and struct fields.
//! (Regression: `EnumValue` lowering was a stub emitting 0 for EVERY variant,
//! and `Type::EnumVariant` had no elementary mapping.)

use crate::tests::{compile_to_wasm_checked, with_db};
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
