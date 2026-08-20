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

/// An inherited method body dispatches `THIS` against the INSTANCE's type, so
/// a derived override wins.
///
/// Each POU emits its own copy of every method it responds to, inherited ones
/// included, and a `THIS.m()` inside a copy resolves against the POU it was
/// emitted for. Emitting one body per DECLARING POU instead froze those calls
/// to the base: `Base#Template` called `Base#Hook` forever, so a derived
/// override was silently unreachable and the template-method pattern
/// miscompiled with a clean `rk check`.
#[rstest]
fn inherited_body_dispatches_this_against_the_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
            METHOD PUBLIC Hook : DINT
                Hook := 1;
            END_METHOD
            METHOD PUBLIC Sibling : DINT
                Sibling := 1;
            END_METHOD
            METHOD PUBLIC Template : DINT
                (* THIS.m() and a bare sibling call are both virtual *)
                Template := THIS.Hook() * 10 + Sibling();
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Mid EXTENDS Base
            METHOD PUBLIC OVERRIDE Hook : DINT
                Hook := 2;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Leaf EXTENDS Mid
            METHOD PUBLIC OVERRIDE Sibling : DINT
                Sibling := 7;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR b : Base; m : Mid; l : Leaf; END_VAR
            (* base 11, one override 21, two levels 27 (Hook from Mid,
               Sibling from Leaf) *)
            run := b.Template() * 10000 + m.Template() * 100 + l.Template();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 112127, "each instance runs its own override set");
}

/// `SUPER.m()` is explicitly static (IEC 9b/10b): it names the base's method
/// even when the instance overrides it, and even from a further inheritor.
#[rstest]
fn super_stays_static_under_the_new_dispatch(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
            METHOD PUBLIC Hook : INT
                Hook := 1;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Mid EXTENDS Base
            METHOD PUBLIC OVERRIDE Hook : INT
                Hook := 2;
            END_METHOD
            METHOD PUBLIC ViaSuper : INT
                ViaSuper := SUPER.Hook();
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Leaf EXTENDS Mid
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR m : Mid; l : Leaf; END_VAR
            (* SUPER reaches Base#Hook from both, while THIS would give 2 *)
            run := m.ViaSuper() * 10 + l.ViaSuper();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 11, "SUPER is not virtual");
}
