// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Recursion: each call of a POU that may call itself has a frame of its
//! own on the stack for its memory-resident storage, so an activation never
//! sees another's array, STRING, instance or address-taken scalar. Every
//! other POU keeps its storage at static addresses.

use crate::tests::codegen::{compile_to_wasm, run, with_db};
use rstest::*;

/// The report's case: an ARRAY local each activation writes before calling
/// itself. All of them shared one, so every caller read the innermost `0`.
#[rstest]
fn each_call_keeps_its_own_array(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Walker
            METHOD PUBLIC Depth : INT
            VAR_INPUT n : INT; END_VAR
            VAR a : ARRAY[0..1] OF INT; END_VAR
                a[0] := n;
                IF n > 0 THEN
                    THIS.Depth(n := n - 1);
                END_IF;
                Depth := a[0];
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION FDepth : INT
        VAR_INPUT n : INT; END_VAR
        VAR a : ARRAY[0..1] OF INT; END_VAR
            a[0] := n;
            IF n > 0 THEN
                FDepth(n := n - 1);
            END_IF;
            FDepth := a[0];
        END_FUNCTION

        FUNCTION SumTo : INT
        VAR_INPUT n : INT; END_VAR
        VAR keep : ARRAY[0..0] OF INT; END_VAR
            keep[0] := n;
            IF n > 0 THEN
                SumTo := SumTo(n - 1);
            END_IF;
            SumTo := SumTo + keep[0];
        END_FUNCTION

        FUNCTION test : INT
        VAR w : Walker; END_VAR
            test := FDepth(n := 3) * 100 + w.Depth(n := 3) * 10 + SumTo(3);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 336,
        "3 from the function, 3 from the method, 6 from SumTo"
    );
}

/// Two FUNCTIONs calling each other, each with storage of its own.
#[rstest]
fn mutual_recursion(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Even : BOOL
        VAR_INPUT n : INT; END_VAR
        VAR seen : ARRAY[0..0] OF INT; END_VAR
            seen[0] := n;
            IF n = 0 THEN Even := TRUE; ELSE Even := Odd(n - 1); END_IF;
            IF seen[0] <> n THEN Even := FALSE; END_IF;
        END_FUNCTION

        FUNCTION Odd : BOOL
        VAR_INPUT n : INT; END_VAR
        VAR seen : ARRAY[0..0] OF INT; END_VAR
            seen[0] := n;
            IF n = 0 THEN Odd := FALSE; ELSE Odd := Even(n - 1); END_IF;
            IF seen[0] <> n THEN Odd := TRUE; END_IF;
        END_FUNCTION

        FUNCTION test : BOOL
            test := Even(10) AND NOT Even(7) AND Odd(7);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1);
}

/// A STRING result lives in the callee's frame until the caller copies it;
/// two recursive calls nested in one expression each have their snapshot;
/// a STRING local keeps each call's own value across the call below it.
#[rstest]
fn a_recursive_function_returns_strings(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION str_concat : STRING
        VAR_INPUT a : STRING; b : STRING; END_VAR
            {wasm 'str.concat' (params a b) (result str_concat)}
        END_FUNCTION

        FUNCTION Stars : STRING
        VAR_INPUT n : INT; END_VAR
            IF n > 0 THEN
                Stars := str_concat(Stars(n - 1), '*');
            END_IF;
        END_FUNCTION

        FUNCTION Both : STRING
        VAR_INPUT n : INT; END_VAR
            IF n > 0 THEN
                Both := str_concat(Stars(n), str_concat('-', Both(n - 1)));
            END_IF;
        END_FUNCTION

        FUNCTION Wrap : STRING
        VAR_INPUT n : INT; END_VAR
        VAR mine : STRING; END_VAR
            IF n MOD 2 = 0 THEN mine := 'a'; ELSE mine := 'b'; END_IF;
            IF n > 0 THEN Wrap := Wrap(n - 1); END_IF;
            Wrap := str_concat(mine, str_concat(Wrap, ')'));
        END_FUNCTION

        FUNCTION test : BOOL
            test := Stars(3) = '***' AND Both(3) = '***-**-*-' AND Wrap(3) = 'baba))))';
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 1);
}

/// A STRUCT result, returned as the address of the callee's slot in its
/// frame and copied out before the next call can push a frame over it, also
/// into the snapshot an aggregate input is passed as.
#[rstest]
fn a_recursive_function_returns_a_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pair : STRUCT a : DINT; b : DINT; END_STRUCT END_TYPE

        FUNCTION Fib : Pair
        VAR_INPUT n : INT; END_VAR
        VAR prev : Pair; END_VAR
            IF n = 0 THEN
                Fib.a := 0;
                Fib.b := 1;
            ELSE
                prev := Fib(n - 1);
                Fib.a := prev.b;
                Fib.b := prev.a + prev.b;
            END_IF;
        END_FUNCTION

        FUNCTION Total : DINT
        VAR_INPUT p : Pair; n : INT; END_VAR
            IF n = 0 THEN
                Total := p.a + p.b;
            ELSE
                Total := Total(Fib(n), n - 1) + p.a;
            END_IF;
        END_FUNCTION

        FUNCTION test : DINT
        VAR p : Pair; q : Pair; END_VAR
            p := Fib(10);
            q := Fib(5);
            test := p.a * 1000 + q.a * 10 + Total(Fib(0), 2);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    // Fib(10).a = 55, Fib(5).a = 5, and Total(Fib(0), 2) = 3.
    assert_eq!(result, 55 * 1000 + 5 * 10 + 3);
}

/// A caller's local passed down by address, as a VAR_IN_OUT and through
/// `REF()`: the callee writes the caller's frame, not its own.
#[rstest]
fn a_local_passed_down_by_address(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Count : INT
        VAR_INPUT n : INT; END_VAR
        VAR_IN_OUT total : INT; END_VAR
        VAR mine : INT; r : REF_TO INT; END_VAR
            mine := 0;
            r := REF(mine);
            IF n > 0 THEN
                Count(n := n - 1, total := mine);
            END_IF;
            total := total + r^ + n;
            Count := mine;
        END_FUNCTION

        FUNCTION test : INT
        VAR t : INT; END_VAR
            Count(n := 3, total := t);
            test := t;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    // Each level adds its child's total (kept in its own `mine`) and its `n`.
    assert_eq!(result, 6);
}

/// An FB instance held by a recursive METHOD: each call's instance starts
/// over and keeps its own state across the call below it.
#[rstest]
fn an_instance_local_in_a_recursive_method(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR n : INT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK

        CLASS Tree
            METHOD PUBLIC Walk : INT
            VAR_INPUT depth : INT; END_VAR
            VAR c : Counter; below : INT; END_VAR
                c();
                IF depth > 0 THEN
                    below := THIS.Walk(depth := depth - 1);
                END_IF;
                c();
                Walk := c.n * 10 + below;
            END_METHOD
        END_CLASS

        FUNCTION test : INT
        VAR t : Tree; END_VAR
            test := t.Walk(depth := 2);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 60, "every level counted its own two calls");
}

/// A recursion deeper than the stack stops with an exception naming it,
/// instead of running on over the memory past the stack.
#[rstest]
fn too_deep_a_recursion_raises(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Deep : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR big : ARRAY[0..999] OF DINT; END_VAR
            big[0] := n;
            Deep := Deep(n + 1) + big[0];
        END_FUNCTION

        FUNCTION test : DINT
            test := Deep(0);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) =
        crate::tests::codegen::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
    let err = f.call(&mut store, ()).expect_err("a recursion without end");
    let message = crate::tests::codegen::fault_message(&mut store, memory, err);
    assert_eq!(message, "stack overflow: recursion too deep");
}

/// A PROGRAM is only ever started by the host, so its body starts the stack
/// over: a scan the overflow stopped leaves no frames behind for the next.
#[rstest]
fn the_next_scan_starts_the_stack_over(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Deep : DINT
        VAR_INPUT n : DINT; limit : DINT; END_VAR
        VAR big : ARRAY[0..999] OF DINT; END_VAR
            big[0] := n;
            IF n < limit THEN
                Deep := Deep(n + 1, limit) + big[0];
            ELSE
                Deep := big[0];
            END_IF;
        END_FUNCTION

        PROGRAM P
        VAR scans : DINT; END_VAR
        VAR_EXTERNAL got : DINT; END_VAR
            scans := scans + 1;
            IF scans = 1 THEN
                got := Deep(0, 100000);
            ELSE
                got := Deep(0, 10);
            END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL got : DINT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    let first = plc.scan().expect_err("the first scan recurses without end");
    assert!(format!("{first:#}").contains("stack overflow"), "{first:#}");
    for _ in 0..20 {
        plc.scan()
            .expect("a scan after the overflow has the whole stack");
    }
    let globals = plc.read_globals();
    let got = i32::from_le_bytes([globals[0], globals[1], globals[2], globals[3]]);
    assert_eq!(got, 55, "0 + 1 + ... + 10");
}
