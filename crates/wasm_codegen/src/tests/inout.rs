//! VAR_IN_OUT semantics at FB call sites: by-reference binding (a pointer
//! field in the instance struct, stored once per call, auto-dereferenced in the
//! body), aliasing, aggregates, nesting/passthrough, STRING, and the E0234
//! l-value requirement.

use crate::tests::{compile_to_mir_and_wasm, compile_to_wasm_checked, with_db};
use rstest::*;
use runtime::{Config, Plc};

/// Decode the string at the start of the retain band: `[len:i32]` + bytes.
fn read_retain_string(plc: &Plc) -> String {
    let r = plc.read_retain();
    let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
    String::from_utf8_lossy(&r[4..4 + len]).to_string()
}

/// The caller's variable receives the callee's mutation. `d(v := x)` with x=21
/// and `v := v * 2` must leave x = 42. (Regression: with the old value-field
/// model only the copy-in was emitted, so x silently kept 21.)
#[rstest]
fn fb_inout_basic(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; x : INT := 21; END_VAR
            d(v := x);
            test := x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "FB inout by-ref: x becomes 42");
}

/// The positional (non-formal) arg form `d(x)` binds the inout too, and the
/// reference round-trips across two calls (42 -> 84).
#[rstest]
fn fb_inout_positional(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; x : INT := 21; END_VAR
            d(x);
            d(x);
            test := x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 84, "positional inout round-trips: 21->42->84");
}

/// A VAR_IN_OUT bound to a struct field as the caller l-value: the write must
/// land in `p.n` specifically, leaving the sibling field untouched.
#[rstest]
fn fb_inout_field_target(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pair : STRUCT n : INT; other : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; p : Pair; END_VAR
            p.n := 21;
            p.other := 5;
            d(v := p.n);
            test := p.n + p.other;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 47, "writes p.n=42, leaves p.other=5 -> 47");
}

/// The behavioral discriminator between by-reference and copy-in/copy-out:
/// the SAME caller variable is bound to two VAR_IN_OUT params. The body writes
/// through the first (`a := 100`) and then reads the second (`b`). Under
/// by-reference both params alias the one variable, so `b` observes 100 live.
/// Under copy-in/copy-out each param would be an independent snapshot and `b`
/// would still read the original value — this test can only pass if the
/// binding is a real reference.
#[rstest]
fn fb_inout_aliasing(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK aliaser
            VAR_IN_OUT a : INT; b : INT; END_VAR
            VAR_OUTPUT seen : INT; END_VAR
            a := 100;
            seen := b;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR fb : aliaser; x : INT := 7; END_VAR
            fb(a := x, b := x);
            test := fb.seen;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 100,
        "by-reference: writing through `a` is visible via `b` (same var); \
         copy-in/copy-out would report 7"
    );
}

/// A struct VAR_IN_OUT is passed by reference (one address stored, no value
/// copy of the aggregate). The body mutates fields through the reference and
/// the caller observes it — the old value model silently dropped aggregates.
#[rstest]
fn fb_inout_aggregate(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK bump
            VAR_IN_OUT v : Vec2; END_VAR
            v.x := v.x + 10;
            v.y := v.y + 20;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR fb : bump; p : Vec2; END_VAR
            p.x := 1;
            p.y := 2;
            fb(v := p);
            test := p.x + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 33, "struct inout by-ref: (1+10) + (2+20) = 33");
}

/// A nested FB (an instance that is itself a member of the calling FB) bound
/// to an inout arg that is another member of the outer FB: the call site runs
/// through the dynamic `this + offset` FbCall branch and must store the
/// member's address, not its value.
#[rstest]
fn fb_inout_nested(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK outer
            VAR d : doubler; x : INT; END_VAR
            VAR_OUTPUT res : INT; END_VAR
            x := 21;
            d(v := x);
            res := x;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR o : outer; END_VAR
            o();
            test := o.res;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "nested FB inout on an outer member: 21 -> 42");
}

/// Inout PASSTHROUGH: an FB forwards its OWN VAR_IN_OUT param to a nested FB's
/// VAR_IN_OUT. The outer's field holds a pointer to the original caller's
/// variable; forwarding must pass that stored pointer along (not the address
/// of the pointer slot), so the innermost mutation reaches the original
/// variable two frames up.
#[rstest]
fn fb_inout_passthrough(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK forwarder
            VAR_IN_OUT w : INT; END_VAR
            VAR d : doubler; END_VAR
            d(v := w);
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : forwarder; x : INT := 21; END_VAR
            f(w := x);
            test := x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "forwarded inout mutates the original variable");
}

/// STRING VAR_IN_OUT on an FB: the callee writes through the reference into
/// the caller's buffer.
#[rstest]
fn fb_inout_string(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK renamer
            VAR_IN_OUT s : STRING; END_VAR
            s := 'hi-fb-inout';
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
        VAR fb : renamer; END_VAR
            s := 'before';
            fb(s := s);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "hi-fb-inout");
}

/// E0234: a VAR_IN_OUT argument must be an l-value. A literal or expression
/// has no address to bind — previously this compiled silently and the callee
/// dereferenced a garbage address.
#[rstest]
fn fb_inout_non_lvalue_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION fn2 : INT
            VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
            fn2 := io;
        END_FUNCTION

        FUNCTION test : INT
        VAR d : doubler; x : INT; END_VAR
            d(v := 5);
            test := fn2(io := x + 1);
        END_FUNCTION
    "#;
    let file = super::add_source(&mut with_db, source);
    let diags = hir::check::diagnostics_for_file(&with_db, file);
    let e0234_count = diags
        .iter()
        .filter(|d| {
            matches!(
                &d.diagnostic.code,
                Some(auto_lsp::lsp_types::NumberOrString::String(s)) if s == "E0234"
            )
        })
        .count();
    assert_eq!(
        e0234_count,
        2,
        "both the FB literal and the FUNCTION expression inout args must be \
         rejected with E0234, got: {diags:?}"
    );
    assert_eq!(diags.len(), 2, "no other diagnostics expected: {diags:?}");
}

/// FUNCTION VAR_IN_OUT bound to a plain scalar FUNCTION-local: the arg must be
/// forced into linear memory so its address exists. (Regression: the
/// address-taken pass only covered FB calls, so `fn2(x)` emitted an
/// addressless AddrOf -> invalid wasm.)
#[rstest]
fn fn_inout_scalar_local(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fn2 : INT
        VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
            fn2 := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR x : INT := 10; END_VAR
            fn2(x);
            fn2(io := x);
            test := x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 12, "two inout calls increment x: 10 -> 12");
}

/// FUNCTION VAR_IN_OUT with a STRUCT: by-reference, the callee mutates the
/// caller's fields through the pointer.
#[rstest]
fn fn_inout_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION bump : INT
        VAR_IN_OUT v : Vec2; END_VAR
            v.x := v.x + 10;
            v.y := v.y + 20;
            bump := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR p : Vec2; END_VAR
            p.x := 1;
            p.y := 2;
            bump(v := p);
            test := p.x + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 33, "struct inout on a FUNCTION: (1+10) + (2+20)");
}

/// FUNCTION VAR_IN_OUT with an ARRAY: element writes through the reference.
#[rstest]
fn fn_inout_array(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION dbl : INT
        VAR_IN_OUT arr : ARRAY[0..1] OF INT; END_VAR
            arr[0] := arr[0] * 2;
            arr[1] := arr[1] * 2;
            dbl := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR a : ARRAY[0..1] OF INT; END_VAR
            a[0] := 3;
            a[1] := 4;
            dbl(arr := a);
            test := a[0] + a[1];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 14, "array inout on a FUNCTION: 6 + 8");
}
