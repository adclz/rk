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

/// A call in a local's initializer is a call of the body: it runs at each
/// call, before the first statement. `Keep` reaches itself only through
/// the initializer of `below`; with one `mine` for every call, each would
/// read the one below's.
#[rstest]
fn a_call_in_an_initializer_makes_a_body_recursive(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Down : DINT
        VAR_INPUT n : DINT; END_VAR
            IF n > 0 THEN Down := Keep(n - 1); ELSE Down := 0; END_IF;
        END_FUNCTION

        FUNCTION Keep : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR
            mine : ARRAY[0..0] OF DINT;
            below : DINT := Down(n);
        END_VAR
            Keep := below + mine[0] * 100 + n;
            mine[0] := n;
        END_FUNCTION

        FUNCTION test : DINT
            test := Keep(2);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 3, "0 + 1 + 2, every call reading its own `mine`");
}

/// A call through an interface parameter reaches every implementer: here
/// the method that calls `Drive` back, in the copy of `Drive` for `Walker`.
#[rstest]
fn a_call_through_an_interface_makes_a_body_recursive(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IStep
            METHOD Step : DINT
            VAR_INPUT n : DINT; END_VAR
            END_METHOD
        END_INTERFACE

        FUNCTION Drive : DINT
        VAR_INPUT it : IStep; n : DINT; END_VAR
        VAR mine : ARRAY[0..0] OF DINT; END_VAR
            mine[0] := n;
            Drive := it.Step(n := n) + mine[0] * 100;
        END_FUNCTION

        CLASS Walker IMPLEMENTS IStep
            METHOD PUBLIC Step : DINT
            VAR_INPUT n : DINT; END_VAR
                IF n > 0 THEN
                    Step := Drive(it := THIS, n := n - 1);
                ELSE
                    Step := 0;
                END_IF;
            END_METHOD
        END_CLASS

        FUNCTION test : DINT
        VAR w : Walker; END_VAR
            test := Drive(it := w, n := 2);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 300,
        "100 from the call below, 200 from its own `mine`"
    );
}

/// An instance called through a VAR_IN_OUT runs its block's body: the body
/// reaches itself through `Visit`, and its VAR_TEMP is each call's own.
#[rstest]
fn an_instance_called_through_a_var_in_out_makes_a_body_recursive(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Node
        VAR_INPUT depth : DINT; END_VAR
        VAR_OUTPUT total : DINT; END_VAR
        VAR peer : REF_TO Node; END_VAR
        VAR_TEMP buf : ARRAY[0..0] OF DINT; END_VAR
            buf[0] := depth;
            total := 0;
            IF peer <> NULL THEN
                total := Visit(c := peer^, d := depth - 1);
            END_IF;
            total := total + buf[0] * 100;
        END_FUNCTION_BLOCK

        FUNCTION Visit : DINT
        VAR_IN_OUT c : Node; END_VAR
        VAR_INPUT d : DINT; END_VAR
            c(depth := d);
            Visit := c.total;
        END_FUNCTION

        FUNCTION test : DINT
        VAR a : Node; b : Node; END_VAR
            a.peer := REF(b);
            a(depth := 2);
            test := a.total;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 300, "100 from `b`, 200 from `a`'s own `buf`");
}

/// `SUPER.m()` runs the base's method, whose `THIS.m()` runs the override
/// again: the base's method is on a cycle through both.
#[rstest]
fn a_cycle_through_super_method(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS Base
            METHOD PUBLIC Walk : DINT
            VAR_INPUT n : DINT; END_VAR
            VAR mine : ARRAY[0..0] OF DINT; END_VAR
                mine[0] := n;
                IF n > 0 THEN
                    Walk := THIS.Walk(n := n - 1);
                END_IF;
                Walk := Walk * 10 + mine[0];
            END_METHOD
        END_CLASS

        CLASS Derived EXTENDS Base
            METHOD PUBLIC OVERRIDE Walk : DINT
            VAR_INPUT n : DINT; END_VAR
                Walk := SUPER.Walk(n := n);
            END_METHOD
        END_CLASS

        FUNCTION test : DINT
        VAR d : Derived; END_VAR
            test := d.Walk(n := 2);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 12, "each call appends its own `mine`: 0, 1, 2");
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

/// A frame bigger than 64 KiB fits: the stack holds the largest frame and
/// 64 KiB more. The stack was 64 KiB, so a function with such a frame
/// raised `stack overflow` on its first call, before it recursed. The last
/// element of the array is the far end of the frame.
#[rstest]
fn a_frame_bigger_than_64_kib_fits(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Big : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR cells : ARRAY[0..19999] OF DINT; END_VAR
            cells[19999] := n + 7;
            IF n > 0 THEN
                Big := Big(n - 1);
            ELSE
                Big := cells[19999];
            END_IF;
        END_FUNCTION

        FUNCTION test : DINT
            test := Big(0);
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 7, "the call wrote and read the end of its frame");
}

/// `stack_size` from `config.toml` is the stack's size: a recursion 30 deep
/// with 4000-byte frames is more than the largest frame and 64 KiB, and fits
/// in 1 MiB.
#[rstest]
fn a_configured_stack_size_is_the_stack(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Depth : DINT
        VAR_INPUT n : DINT; END_VAR
        VAR cells : ARRAY[0..999] OF DINT; END_VAR
            cells[999] := n;
            IF n > 0 THEN
                Depth := Depth(n - 1) + 1;
            ELSE
                Depth := cells[999];
            END_IF;
        END_FUNCTION

        FUNCTION test : DINT
            test := Depth(30);
        END_FUNCTION
    "#;
    let (mut mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) =
        crate::tests::codegen::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
    let err = f
        .call(&mut store, ())
        .expect_err("too deep for the default");
    let message = crate::tests::codegen::fault_message(&mut store, memory, err);
    assert_eq!(message, "stack overflow: recursion too deep");

    mir.stack_size = Some(1 << 20);
    let wasm =
        wasm_codegen::generate_wasm(&with_db, &crate::tests::codegen::export_everything(&mir))
            .finish();
    let result: i32 = crate::tests::codegen::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 30, "30 frames fit in 1 MiB");
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

/// An instance of a block with no variables takes no bytes, and the frame
/// of a recursive function holding one had no size: the code generator
/// gives a frame base only to a function whose frame pushes something,
/// and panicked on the frame local. The empty instance takes a slot of the
/// frame now, and the function runs. A cycle through `SUPER()` was the
/// shape the fuzzer found it in; it compiles, and is not run, since it
/// never returns.
#[rstest]
fn a_recursive_function_holding_an_empty_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Empty
        END_FUNCTION_BLOCK

        FUNCTION Depth : INT
        VAR_INPUT n : INT; END_VAR
        VAR e : Empty; END_VAR
            e();
            IF n > 0 THEN
                Depth := Depth(n := n - 1) + 1;
            END_IF;
        END_FUNCTION

        FUNCTION_BLOCK Base
            Kick();
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
            SUPER();
        END_FUNCTION_BLOCK

        FUNCTION Kick : INT
        VAR d : Derived; END_VAR
            d();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    assert_eq!(super::execute_wasm::<i32, i32>(&wasm, "Depth", 4), 4);
}

/// Every part a frame can hold, in recursive FUNCTIONs, METHODs and FB
/// bodies: the inputs copied in, the result and the locals, and what the
/// calls they make need while they run. MIR checks each frame it lays out
/// against the one HIR plans, which `rk check` measures the stack against
/// (E1430), so this compiles only if the two agree.
#[rstest]
fn every_part_of_a_frame_is_where_hir_plans_it(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pair : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE

        FUNCTION Take : DINT
        VAR_INPUT p : Pair; END_VAR
            Take := p.a + p.b;
        END_FUNCTION

        FUNCTION Outs : DINT
        VAR_INPUT x : DINT; END_VAR
        VAR_OUTPUT o : DINT; s : STRING[30]; f : BOOL; small : INT; END_VAR
            o := x;
            s := 'out';
            f := x > 0;
            small := 7;
            Outs := x;
        END_FUNCTION

        FUNCTION Name : STRING[12]
        VAR_INPUT n : DINT; END_VAR
            IF n MOD 2 = 0 THEN Name := 'even'; ELSE Name := 'odd'; END_IF;
        END_FUNCTION

        FUNCTION Size : DINT
        VAR_INPUT s : STRING[40]; END_VAR
            Size := 1;
        END_FUNCTION

        FUNCTION Idx : DINT
        VAR_INPUT n : DINT; END_VAR
            Idx := n MOD 2;
        END_FUNCTION

        FUNCTION_BLOCK Gate
        VAR_INPUT i : BOOL; END_VAR
        VAR_OUTPUT q : BOOL; END_VAR
            q := i;
        END_FUNCTION_BLOCK

        FUNCTION Deep : DINT
        VAR_INPUT
            n : DINT;
            label : STRING[20];
            k : INT;
        END_VAR
        VAR_EXTERNAL bit3 : BOOL; bit4 : BOOL; END_VAR
        VAR
            pair : Pair;
            cells : ARRAY[0..2] OF DINT;
            text : STRING[16];
            wide : DINT;
            flag : BOOL;
            names : ARRAY[0..1] OF STRING[12];
            gate : Gate;
            r : REF_TO INT;
        END_VAR
        VAR_TEMP scratch : ARRAY[0..1] OF LREAL; END_VAR
            r := REF(k);
            pair.a := n;
            pair.b := 1;
            wide := Take(pair);
            wide := Outs(x := n);
            wide := Outs(x := n, NOT f => flag);
            wide := Outs(x := n, small => wide);
            wide := Outs(x := n, f => bit3);
            gate(i := flag, NOT q => flag);
            gate(i := flag, q => bit4);
            names[0] := 'even';
            names[1] := 'odd';
            CASE Name(n) OF
                'even': cells[0] := 2;
            ELSE
                cells[0] := 1;
            END_CASE;
            CASE names[Idx(n)] OF
                'odd': cells[1] := 3;
            END_CASE;
            wide := Size(Name(n));
            IF Name(n) = 'even' THEN cells[2] := 4; END_IF;
            IF n > 0 THEN
                Deep := Deep(n - 1, label, k) + cells[0];
            ELSE
                Deep := cells[0];
            END_IF;
        END_FUNCTION

        FUNCTION_BLOCK Walker
            METHOD PUBLIC Depth : Pair
            VAR_INPUT n : DINT; tag : STRING[8]; END_VAR
            VAR p : Pair; END_VAR
                p.a := n;
                p.b := Take(p);
                IF n > 0 THEN
                    Depth := THIS.Depth(n - 1, tag);
                END_IF;
                Depth.a := Depth.a + p.a;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Node
        VAR_INPUT depth : DINT; END_VAR
        VAR_OUTPUT total : DINT; END_VAR
        VAR next : REF_TO Node; END_VAR
        VAR_TEMP buf : ARRAY[0..3] OF DINT; pair : Pair; END_VAR
            buf[0] := depth;
            pair.a := depth;
            buf[1] := Take(pair);
            IF next <> NULL THEN
                next^(depth := depth - 1);
                total := next^.total + buf[0];
            ELSE
                total := buf[0];
            END_IF;
        END_FUNCTION_BLOCK

        FUNCTION SumA : DINT
        VAR_INPUT a : ARRAY[*] OF DINT; i : DINT; END_VAR
            IF i < 0 THEN
                SumA := 0;
            ELSE
                SumA := a[i] + SumA(a, i - 1);
            END_IF;
        END_FUNCTION

        FUNCTION Count : DINT
        VAR_INPUT xs : DINT...; END_VAR
        VAR keep : ARRAY[0..0] OF DINT; END_VAR
            keep[0] := ...xs+;
            IF keep[0] > 3 THEN Count := Count(1, 1) + keep[0]; ELSE Count := keep[0]; END_IF;
        END_FUNCTION

        FUNCTION test : DINT
        VAR
            w : Walker;
            pr : Pair;
            a : Node;
            b : Node;
            three : ARRAY[0..2] OF DINT := [1, 2, 3];
            m : ARRAY[0..1, 0..1] OF DINT := [1, 2, 3, 4];
        END_VAR
            a.next := REF(b);
            a(depth := 2);
            pr := w.Depth(2, 'y');
            test := Deep(2, 'x', 1) + pr.a + a.total;
            test := test + SumA(three, 2) + SumA(m[1], 1) + Count(5, 6, 7);
        END_FUNCTION

        PROGRAM P
        VAR got : DINT; END_VAR
            got := test();
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            lamps AT %QW0 : WORD;
            bit3 AT %QX0.3 : BOOL;
            bit4 AT %QX0.4 : BOOL;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    assert_eq!(
        super::execute_wasm::<(), i32>(&wasm, "test", ()),
        5 + 3 + 3 + 6 + 7 + 20,
        "Deep, Depth, the chain of Nodes, both copies of SumA, Count"
    );
}
