// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `**` executes: it lowers to the grafted `libm` pow, so these run the
//! whole path — graft, call resolution, operand casting — under wasmtime.
//! The hir suite pins what is REJECTED; this pins what the accepted
//! shapes compute.

use rstest::rstest;

use super::with_db;

#[rstest]
fn power_of_real_literals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
            test := 2.0 ** 3.0;
        END_FUNCTION
    "#;
    let result: f32 = super::run(&mut with_db, source, "test", ());
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
    let result: f32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 9.0);
}

/// A LITERAL integer exponent resolves in its own context (to INT) and is
/// cast to the base at the call. This is the shape that ICEd — "numeric
/// literal type was never resolved" — whenever nothing resolved the literal.
#[rstest]
fn power_with_literal_exponent(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
        VAR
            x : REAL := 3.0;
        END_VAR
            test := x ** 2;
        END_FUNCTION
    "#;
    let result: f32 = super::run(&mut with_db, source, "test", ());
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
    let result: f32 = super::run(&mut with_db, source, "test", ());
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
    let result: f64 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1024.0);
}

/// `**` over constants is a value `__init` stores: a global's, a member's of
/// a program and of a function block that is a global, and a VAR_CONFIG
/// value. Each is what the program computes in its body. `**` is a call to
/// the math builtin, and a call in such a value stopped the compiler.
#[rstest]
fn a_power_is_a_value_init_stores(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT k : REAL := 3.0; END_VAR
        VAR_GLOBAL g : REAL := 2.0 ** k; gf : Fg; END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM P1 WITH T : Main;
        END_RESOURCE
        VAR_CONFIG P1.b.v : REAL := 2.0 ** 3.0; END_VAR
        END_CONFIGURATION

        FUNCTION_BLOCK Fb
        VAR v : REAL; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Fg
        VAR_EXTERNAL CONSTANT k : REAL; END_VAR
        VAR m : REAL := 2.0 ** k; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR_EXTERNAL CONSTANT k : REAL; END_VAR
        VAR_EXTERNAL g : REAL; gf : Fg; END_VAR
        VAR p : REAL := 2.0 ** k; b : Fb; same : BOOL; END_VAR
            same := p = 2.0 ** k AND g = 2.0 ** k AND gf.m = 2.0 ** k AND b.v = 2.0 ** k;
        END_PROGRAM
    "#;
    let (_mir, wasm) = super::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = super::TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = debug_format::DebugInfo::from_wasm(&wasm);
    match plc.read_var(&info, "P1.same") {
        Some(debug_format::VarValue::Bool(same)) => assert!(same, "a value differs from 2.0 ** k"),
        other => panic!("P1.same is {other:?}"),
    }
}
