//! FUNCTION aggregate (struct/array) `VAR_INPUT` args: copy semantics, like
//! FUNCTION_BLOCK inputs. The caller copies the arg into a scratch local at
//! call entry (`MirExpr::CopyIntoScratch`) and the callee receives a pointer
//! to that snapshot — so callee-side writes and later caller-side mutations
//! never leak across the call boundary. (Regression: aggregates previously
//! lowered as a bogus 1-slot "value" and the module failed wasm validation.)

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// The audit probe: a STRUCT passed by value into a FUNCTION.
#[rstest]
fn fn_struct_input(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION take_pt : INT
        VAR_INPUT p : Point; END_VAR
            take_pt := p.x + p.y;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 10;
            s.y := 20;
            test := take_pt(p := s);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 30, "struct input by value: 10 + 20");
}

/// An ARRAY passed by value into a FUNCTION.
#[rstest]
fn fn_array_input(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum4 : INT
        VAR_INPUT a : ARRAY[0..3] OF INT; END_VAR
            sum4 := a[0] + a[1] + a[2] + a[3];
        END_FUNCTION

        FUNCTION test : INT
        VAR arr : ARRAY[0..3] OF INT; END_VAR
            arr[0] := 1;
            arr[1] := 2;
            arr[2] := 3;
            arr[3] := 4;
            test := sum4(a := arr);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 10, "array input by value: 1+2+3+4");
}

/// Value semantics: the callee writes to its input (L0303 lint, legal) — the
/// mutation lands in the call-entry snapshot, never in the caller's struct.
#[rstest]
fn fn_struct_input_callee_write_invisible(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION clobber : INT
        VAR_INPUT p : Point; END_VAR
            p.x := 999;
            clobber := p.x;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 10;
            s.y := 20;
            clobber(p := s);
            test := s.x + s.y;
        END_FUNCTION
    "#;
    // Unchecked compile: writing to a VAR_INPUT raises the L0303 lint.
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 30,
        "callee wrote its snapshot, caller's s unchanged"
    );
}

/// Snapshot semantics under aliasing: the SAME struct is bound to a VAR_INPUT
/// and a VAR_IN_OUT. The body mutates through the inout reference first, then
/// reads the input — which must still show the call-entry values (the copy
/// was taken before the call, exactly like a C-emitting compiler/FB copy-in).
#[rstest]
fn fn_struct_input_snapshot_aliasing(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION probe : INT
        VAR_INPUT snap : Point; END_VAR
        VAR_IN_OUT live : Point; END_VAR
            live.x := 100;
            probe := snap.x;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 7;
            test := probe(snap := s, live := s);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 7,
        "input is a call-entry snapshot: sees 7, not the inout's 100"
    );
}

/// Forwarding: a function passes its own aggregate input on to another
/// function — each hop takes a fresh snapshot.
#[rstest]
fn fn_struct_input_forwarding(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION inner_sum : INT
        VAR_INPUT q : Point; END_VAR
            inner_sum := q.x + q.y;
        END_FUNCTION

        FUNCTION outer_fwd : INT
        VAR_INPUT p : Point; END_VAR
            outer_fwd := inner_sum(q := p);
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 4;
            s.y := 5;
            test := outer_fwd(p := s);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 9, "forwarded struct input: 4 + 5");
}

/// Aggregate input combined with a scalar input and a discarded output in the
/// same call — arg positions must all line up.
#[rstest]
fn fn_mixed_aggregate_scalar_discard(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION mixed : INT
        VAR_INPUT p : Point; k : INT; END_VAR
        VAR_OUTPUT dbg : INT; END_VAR
            dbg := p.x;
            mixed := p.x * k + p.y;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 3;
            s.y := 1;
            test := mixed(p := s, k := 10);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 31, "3*10 + 1, dbg discarded");
}

/// Aggregate input on a call made from inside an FB body — the scratch lands
/// in the `$__body__` function's locals.
#[rstest]
fn fn_struct_input_from_fb_body(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION take_pt : INT
        VAR_INPUT p : Point; END_VAR
            take_pt := p.x + p.y;
        END_FUNCTION

        FUNCTION_BLOCK caller
        VAR pt : Point; END_VAR
        VAR_OUTPUT res : INT; END_VAR
            pt.x := 8;
            pt.y := 9;
            res := take_pt(p := pt);
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR c : caller; END_VAR
            c();
            test := c.res;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 17, "struct input from an FB body: 8 + 9");
}
