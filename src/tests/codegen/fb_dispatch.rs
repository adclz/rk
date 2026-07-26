//! Calling a function block through something other than a bare local name.
//!
//! The receiver of an FB invocation used to be collapsed to a single ident and
//! re-resolved as a path *root*. `PathExpr::ident` returns the TRAILING
//! segment, so `cells[2]()` lost its subscript and ran element 0, and `h.a()`
//! looked up `a` in the caller's own frame — where it does not exist — and the
//! codegen's `_ => return` then discarded the whole statement in silence.
//!
//! `ARRAY[0..n] OF TON` with one timer per axis is the canonical shape this
//! broke: every axis shared element 0's timer.

use crate::tests::codegen::{compile_to_wasm_checked, execute_wasm, with_db};
use rstest::*;

const CELL: &str = r#"
    FUNCTION_BLOCK Cell
    VAR
        v : INT;
    END_VAR
        v := v + 1;
    END_FUNCTION_BLOCK
"#;

/// A constant subscript must select that element and no other.
#[rstest]
#[case(0, 1, 0, 0)]
#[case(1, 0, 1, 0)]
#[case(2, 0, 0, 1)]
fn constant_subscript_calls_that_element(
    mut with_db: db::RootDatabase,
    #[case] which: usize,
    #[case] e0: i32,
    #[case] e1: i32,
    #[case] e2: i32,
) {
    let source = format!(
        r#"{CELL}
        FUNCTION run : DINT
        VAR
            cells : ARRAY[0..2] OF Cell;
        END_VAR
            cells[{which}]();
            run := cells[0].v * 10000 + cells[1].v * 100 + cells[2].v;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm_checked(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, e0 * 10000 + e1 * 100 + e2, "cells[{which}]() ran the wrong element");
}

/// A runtime subscript takes the dynamic path — the address is computed into a
/// scratch local rather than folded.
#[rstest]
fn runtime_subscript_calls_each_element(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{CELL}
        FUNCTION run : DINT
        VAR
            cells : ARRAY[0..2] OF Cell;
            i : INT;
        END_VAR
            FOR i := 0 TO 2 DO
                cells[i]();
            END_FOR;
            run := cells[0].v * 10000 + cells[1].v * 100 + cells[2].v;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm_checked(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 10101, "each element ticked exactly once");
}

/// A non-zero lower bound must not be mistaken for a zero-based one.
#[rstest]
fn non_zero_lower_bound_subscript(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{CELL}
        FUNCTION run : DINT
        VAR
            cells : ARRAY[1..3] OF Cell;
        END_VAR
            cells[3]();
            run := cells[1].v * 10000 + cells[2].v * 100 + cells[3].v;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm_checked(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "cells[3] is the LAST element, not the first");
}

/// Calling an FB member of another FB instance used to be a silent no-op.
#[rstest]
fn call_on_a_function_block_member(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{CELL}
        FUNCTION_BLOCK Holder
        VAR
            a : Cell;
            b : Cell;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            h : Holder;
        END_VAR
            h.a();
            h.a();
            h.b();
            run := h.a.v * 100 + h.b.v;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm_checked(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 201, "a ticked twice, b once — and they are distinct");
}

/// Two levels of member access, and an array of FBs held as a member.
#[rstest]
fn call_on_a_nested_member_and_member_array(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{CELL}
        FUNCTION_BLOCK Mid
        VAR
            leaf : Cell;
            row : ARRAY[0..1] OF Cell;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Top
        VAR
            mid : Mid;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            t : Top;
        END_VAR
            t.mid.leaf();
            t.mid.row[1]();
            run := t.mid.leaf.v * 10000 + t.mid.row[0].v * 100 + t.mid.row[1].v;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm_checked(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 10001, "leaf and row[1] ticked; row[0] untouched");
}

/// Inputs and outputs must be written to and read from the SELECTED element,
/// not element 0 — the field offsets are relative to the receiver's base.
#[rstest]
fn inputs_and_outputs_follow_the_subscript(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Scale
        VAR_INPUT
            n : INT;
        END_VAR
        VAR_OUTPUT
            out : INT;
        END_VAR
            out := n * 2;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            s : ARRAY[0..2] OF Scale;
        END_VAR
            s[2](n := 21);
            run := s[0].out * 10000 + s[1].out * 100 + s[2].out;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 42, "the input and the output both landed on element 2");
}

/// An FB instance held in a VAR_GLOBAL: the receiver is a `Global` place, which
/// codegen used to discard through `_ => return`.
#[rstest]
fn call_on_a_global_instance(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{CELL}
        PROGRAM P
        VAR RETAIN
            seen : DINT;
        END_VAR
        VAR_EXTERNAL
            g : Cell;
        END_VAR
            g();
            seen := g.v;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                g : Cell;
            END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#
    );
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, &source);
    let mut plc = runtime::Plc::load(&wasm, runtime::Config::default()).expect("load");
    plc.run(3).expect("scans");
    let seen = i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    assert_eq!(seen, 3, "the global instance ticked once per scan");
}
