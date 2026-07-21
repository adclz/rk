//! FUNCTION `VAR_OUTPUT` at call sites: bound (`o => x`, a live pointer to the
//! caller's l-value) and DISCARDED (omitted at the call — legal per E0233's
//! rules; the callee's pointer param is satisfied by a synthesized scratch
//! local in the caller, see `build_call_args`).

use crate::tests::codegen::{compile_to_wasm_checked, with_db};
use rstest::*;

/// Bound output `o => x`: the callee writes through a live pointer to `x`.
/// Under the other toolchains by-reference model the callee's `o` IS the caller's `x`,
/// so a read-before-write observes the caller's current value (41 -> 42).
#[rstest]
fn bound_output_live_pointer(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION g : INT
        VAR_OUTPUT o : INT; END_VAR
            o := o + 1;
            g := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR x : INT := 41; END_VAR
            g(o => x);
            test := x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "output aliases the caller's x: 41 + 1");
}

/// Discarded output: the call omits `o` entirely. The module must still
/// validate (the callee's pointer param gets a scratch address) and the
/// return value must be correct. (Regression: previously the call pushed one
/// arg too few and the whole module failed wasm validation.)
#[rstest]
fn discarded_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fn : INT
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT o : INT; END_VAR
            o := a * 2;
            fn := a + 1;
        END_FUNCTION

        FUNCTION test : INT
            test := fn(a := 5);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 6, "discarded output: call still works, returns a+1");
}

/// One output bound, one discarded — the discarded one must not shift the
/// bound one's arg position (args are positional in declaration order).
#[rstest]
fn partially_discarded_outputs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION split : INT
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT dbl : INT; tri : INT; END_VAR
            dbl := a * 2;
            tri := a * 3;
            split := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR t : INT; END_VAR
            split(a := 5, tri => t);
            test := t;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 15, "dbl discarded, tri bound: 5*3");
}

/// Two calls with discarded outputs in one caller — each call site gets its
/// own scratch local.
#[rstest]
fn discarded_outputs_two_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fn : INT
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT o : INT; END_VAR
            o := a;
            fn := a + 1;
        END_FUNCTION

        FUNCTION test : INT
            test := fn(a := 1) + fn(a := 10);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 13, "2 + 11");
}

/// A discarded output on a call made from inside an FB body (the scratch
/// local lands in the `$__body__` function's locals).
#[rstest]
fn discarded_output_from_fb_body(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fn : INT
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT o : INT; END_VAR
            o := a * 2;
            fn := a + 1;
        END_FUNCTION

        FUNCTION_BLOCK caller
        VAR_OUTPUT res : INT; END_VAR
            res := fn(a := 20);
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR c : caller; END_VAR
            c();
            test := c.res;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 21, "discarded output inside an FB body: 20+1");
}

/// A discarded STRING output: the callee's (addr, cap) output pair points at
/// a scratch buffer with a real capacity, so the write is bounded and
/// harmless.
#[rstest]
fn discarded_string_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION tag : INT
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT label : STRING; END_VAR
            label := 'discarded-label';
            tag := a * 2;
        END_FUNCTION

        FUNCTION test : INT
            test := tag(a := 21);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "discarded STRING output: call works, returns 42");
}

/// Bound STRUCT output: the callee writes fields through its output pointer;
/// the caller's struct receives them.
#[rstest]
fn bound_struct_output(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION mk : INT
        VAR_INPUT seed : INT; END_VAR
        VAR_OUTPUT v : Vec2; END_VAR
            v.x := seed;
            v.y := seed * 2;
            mk := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR got : Vec2; END_VAR
            mk(seed := 5, v => got);
            test := got.x + got.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 15, "struct output written through the pointer: 5 + 10");
}

/// Bound ARRAY output: element writes through the output pointer land in the
/// caller's array.
#[rstest]
fn bound_array_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fill : INT
        VAR_INPUT seed : INT; END_VAR
        VAR_OUTPUT arr : ARRAY[0..1] OF INT; END_VAR
            arr[0] := seed;
            arr[1] := seed * 10;
            fill := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR a : ARRAY[0..1] OF INT; END_VAR
            fill(seed := 3, arr => a);
            test := a[0] + a[1];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 33, "array output written through the pointer: 3 + 30");
}

/// Bound STRING output, verified through the runtime: the callee's
/// capacity-bounded write lands in the caller's buffer.
#[rstest]
fn bound_string_output(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};

    let source = r#"
        FUNCTION name_it : INT
        VAR_OUTPUT label : STRING; END_VAR
            label := 'fn-out-str';
            name_it := 0;
        END_FUNCTION

        PROGRAM P
        VAR RETAIN r : STRING; END_VAR
            name_it(label => r);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    let r = plc.read_retain();
    let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
    assert_eq!(String::from_utf8_lossy(&r[4..4 + len]), "fn-out-str");
}

/// Discarded STRUCT output: the scratch local must hold the whole aggregate,
/// so the callee's field writes stay inside it.
#[rstest]
fn discarded_struct_output(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION mk : INT
        VAR_INPUT seed : INT; END_VAR
        VAR_OUTPUT v : Vec2; END_VAR
            v.x := seed;
            v.y := seed * 2;
            mk := seed + 1;
        END_FUNCTION

        FUNCTION test : INT
            test := mk(seed := 9);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 10, "discarded struct output: call works, returns 10");
}

/// Discarded ARRAY output: same — the whole array fits in the scratch local.
#[rstest]
fn discarded_array_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fill : INT
        VAR_INPUT seed : INT; END_VAR
        VAR_OUTPUT arr : ARRAY[0..3] OF INT; END_VAR
            arr[0] := seed;
            arr[3] := seed * 10;
            fill := seed * 2;
        END_FUNCTION

        FUNCTION test : INT
            test := fill(seed := 4);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 8, "discarded array output: call works, returns 8");
}

/// A discarded output on a GENERIC (ANY_*) function: the output's concrete
/// type is only picked during monomorphization, so the scratch falls back to
/// an 8-byte slot that fits any elementary resolution.
#[rstest]
fn discarded_any_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION move2 : INT
        VAR_INPUT in1 : ANY; END_VAR
        VAR_OUTPUT out1 : INTO(in1); END_VAR
            out1 := in1;
            move2 := 7;
        END_FUNCTION

        FUNCTION test : INT
            test := move2(in1 := 42);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 7, "generic fn with discarded INTO output");
}
