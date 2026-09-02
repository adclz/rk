//! FUNCTION `VAR_OUTPUT` at call sites: bound (`o => x`, a live pointer to the
//! caller's l-value) and DISCARDED (omitted at the call — legal per E0233's
//! rules; the callee's pointer param is satisfied by a synthesized scratch
//! local in the caller, see `build_call_args`).

use crate::tests::codegen::{compile_to_wasm, with_db};
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
    let wasm = compile_to_wasm(&mut with_db, source);
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
    let wasm = compile_to_wasm(&mut with_db, source);
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
    let wasm = compile_to_wasm(&mut with_db, source);
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
    let wasm = compile_to_wasm(&mut with_db, source);
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
    let wasm = compile_to_wasm(&mut with_db, source);
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 42,
        "discarded STRING output: call works, returns 42"
    );
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 15,
        "struct output written through the pointer: 5 + 10"
    );
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 33,
        "array output written through the pointer: 3 + 30"
    );
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 10,
        "discarded struct output: call works, returns 10"
    );
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 8, "discarded array output: call works, returns 8");
}

/// A METHOD's `VAR_OUTPUT` rides the same convention as a FUNCTION's: a
/// pointer param the callee writes through. The method signature blocks had
/// drifted from the function one and made the output a plain local — the
/// call still pushed a pointer for it, one value too many on the wasm stack,
/// so the whole module failed validation.
#[rstest]
fn bound_method_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Worker
            METHOD PUBLIC Split : INT
            VAR_OUTPUT rem : INT; END_VAR
                rem := 3;
                Split := 10;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION test : INT
        VAR w : Worker; q : INT; r : INT; END_VAR
            q := w.Split(rem => r);
            test := q * 100 + r;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1003, "return 10, rem 3, both through the call");
}

/// The same method with its output DISCARDED: the signature still has the
/// pointer param, so the call feeds it a scratch.
#[rstest]
fn discarded_method_output(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Worker
            METHOD PUBLIC Split : INT
            VAR_OUTPUT rem : INT; END_VAR
                rem := 3;
                Split := 10;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION test : INT
        VAR w : Worker; END_VAR
            test := w.Split();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 10, "the discarded output landed in a scratch");
}

/// Each call site's discarded-output scratch is its own: the callee reads its
/// output before writing (0 + 1 both times), so a shared scratch would give
/// 12. The FIRST execution of a site reads 0; what a later execution of the
/// SAME site reads is deliberately not pinned here.
#[rstest]
fn discarded_scratch_is_per_call_site(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION g : INT
        VAR_OUTPUT o : INT; END_VAR
            o := o + 1;
            g := o;
        END_FUNCTION
        FUNCTION test : INT
            test := g() * 10 + g();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 11, "two sites, two scratches, both initially zero");
}

/// The `cap` half of a STRING output's (addr, cap) pair is what bounds the
/// callee's write: the callee has no idea the caller's buffer is 4 wide.
/// A 10-byte value lands as its first 4 bytes, len clamped to match.
#[rstest]
fn bound_string_output_truncates_to_capacity(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};
    let source = r#"
        FUNCTION name_it : INT
        VAR_OUTPUT label : STRING; END_VAR
            label := 'fn-out-str';
            name_it := 0;
        END_FUNCTION
        PROGRAM P
        VAR RETAIN r : STRING[4]; END_VAR
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
    assert_eq!(len, 4, "len clamped to the caller's capacity");
    assert_eq!(&r[4..8], b"fn-o", "the write stopped at the boundary");
}

/// A discarded-output call nested inside another call's argument: two live
/// scratches at once. The callee reads its output before writing, so if the
/// inner call's scratch aliased the outer's, the outer would read 5 and
/// return 10.
#[rstest]
fn discarded_output_nested_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION g : INT
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT o : INT; END_VAR
            o := o + a;
            g := o;
        END_FUNCTION
        FUNCTION test : INT
            test := g(a := g(a := 5));
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 5, "each nested call kept its own scratch");
}

/// A `=>` destination in ANOTHER lane: the callee writes its OWN lane through
/// the pointer it is given, so the caller receives the output in a memory
/// scratch and converts it afterwards. Before, the callee's four REAL bytes
/// landed over the first half of the LREAL — check-clean, wrong value.
#[rstest]
fn bound_output_converts_across_lanes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION g : INT
        VAR_OUTPUT r : REAL; d : DINT; END_VAR
            r := 2.5;
            d := -1;
            g := 1;
        END_FUNCTION

        FUNCTION floats : LREAL
        VAR l : LREAL; big : LINT; END_VAR
            g(r => l, d => big);
            floats := l * 10.0;
        END_FUNCTION

        FUNCTION ints : LINT
        VAR l : LREAL; big : LINT; k : INT; END_VAR
            k := g(r => l, d => big);
            ints := big * 10 + k;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let f: f64 = super::execute_wasm(&wasm, "floats", ());
    assert_eq!(f, 25.0, "2.5 into an LREAL");
    let i: i64 = super::execute_wasm(&wasm, "ints", ());
    assert_eq!(i, -9, "-1 into a LINT (-10) plus the return value 1, with the value still on the stack under the copies");
}

/// The converted copy reaches MEMORY destinations too — an array element, a
/// struct field — and a METHOD output takes the same path.
#[rstest]
fn bound_output_converts_into_memory_places_and_from_methods(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT l : LREAL; END_STRUCT; END_TYPE

        FUNCTION g : INT
        VAR_OUTPUT r : REAL; END_VAR
            r := 2.5;
            g := 0;
        END_FUNCTION

        CLASS C
            METHOD PUBLIC m : INT
            VAR_OUTPUT r : REAL; END_VAR
                r := 1.5;
                m := 0;
            END_METHOD
        END_CLASS

        FUNCTION test : LREAL
        VAR arr : ARRAY[0..1] OF LREAL; s : Pt; c : C; l : LREAL; k : INT; END_VAR
            k := g(r => arr[1]);
            k := g(r => s.l);
            k := c.m(r => l);
            test := arr[1] * 100.0 + s.l * 10.0 + l;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: f64 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 276.5, "250 + 25 + 1.5");
}

