//! Functions returning STRUCTs and ARRAYs.
//!
//! An aggregate return follows the STRING convention: the callee holds a
//! static return slot, writes the value into it, and returns the slot's
//! ADDRESS as an i32; the caller copies out of it. Before this ABI existed the
//! two sides disagreed with each other — the callee was emitted with no wasm
//! result while the caller consumed one, so the module FAILED VALIDATION,
//! after `rk compile` had reported success and exited 0. And the callee-side
//! `MakePt := tmp` copied only the first four bytes of the struct.
//!
//! Component-wise assignment (`MakePt.x := a`) additionally needed HIR to
//! type the root of a multi-step self-reference as the RETURN type: recording
//! it as the function type left MIR asking a Function for a struct field.

use crate::tests::codegen::{compile_to_wasm_checked, execute_wasm, with_db};
use rstest::*;

/// The canonical shape: build the result component-wise on the function name.
#[rstest]
fn struct_return_built_component_wise(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION MakePt : Point
        VAR_INPUT a : INT; b : INT; END_VAR
            MakePt.x := a;
            MakePt.y := b;
        END_FUNCTION

        FUNCTION run : INT
        VAR p : Point; END_VAR
            p := MakePt(3, 4);
            run := p.x * 100 + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 304, "x = 3 and y = 4, in their own fields");
}

/// The documented workaround shape: build a temp, assign it whole. This used
/// to compile cleanly and emit an INVALID module — the worst outcome, since
/// the user did exactly what the error message suggested.
#[rstest]
fn struct_return_assigned_whole(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION MakePt : Point
        VAR_INPUT a : INT; b : INT; END_VAR
        VAR tmp : Point; END_VAR
            tmp.x := a;
            tmp.y := b;
            MakePt := tmp;
        END_FUNCTION

        FUNCTION run : INT
        VAR p : Point; END_VAR
            p := MakePt(3, 4);
            run := p.x * 100 + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 304, "the WHOLE struct is copied, not its first field");
}

/// Both fields must survive the copy — a 4-byte copy of an 8-byte struct
/// passes any test that only reads `x`.
#[rstest]
fn the_whole_struct_is_copied_not_its_first_word(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : STRUCT a : DINT; b : DINT; c : DINT; END_STRUCT; END_TYPE

        FUNCTION Make : Wide
        VAR tmp : Wide; END_VAR
            tmp.a := 11;
            tmp.b := 22;
            tmp.c := 33;
            Make := tmp;
        END_FUNCTION

        FUNCTION run : DINT
        VAR w : Wide; END_VAR
            run := 99;
            w := Make();
            run := w.a * 10000 + w.b * 100 + w.c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 112233, "all three DINTs arrive");
}

/// A mid-body RETURN in an aggregate function must still deliver the slot's
/// address — the mid-body arm used to push nothing for any non-scalar return.
#[rstest]
fn early_return_still_delivers_the_aggregate(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION Pick : Point
        VAR_INPUT flip : BOOL; END_VAR
            IF flip THEN
                Pick.x := 1;
                Pick.y := 2;
                RETURN;
            END_IF;
            Pick.x := 7;
            Pick.y := 8;
        END_FUNCTION

        FUNCTION run : INT
        VAR p : Point; q : Point; END_VAR
            p := Pick(TRUE);
            q := Pick(FALSE);
            run := p.x * 1000 + p.y * 100 + q.x * 10 + q.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1278, "early return 1,2; fall-through 7,8");
}

/// An ARRAY return takes the same slot-and-pointer path as a STRUCT.
///
/// (Through a named type: the grammar accepts only a type name as a function
/// return type, and rejects an inline `ARRAY[0..2] OF INT` there with a
/// syntax error — loud, and a separate question from this ABI.)
#[rstest]
fn array_return(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Triple3 : ARRAY[0..2] OF INT; END_TYPE

        FUNCTION Triple : Triple3
        VAR_INPUT seed : INT; END_VAR
        VAR tmp : Triple3; k : INT; END_VAR
            FOR k := 0 TO 2 DO
                tmp[k] := seed + k;
            END_FOR;
            Triple := tmp;
        END_FUNCTION

        FUNCTION run : INT
        VAR a : Triple3; END_VAR
            a := Triple(5);
            run := a[0] * 100 + a[1] * 10 + a[2];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 567, "elements 5, 6, 7");
}

/// The result must be a COPY, not a view of the callee's slot: a second call
/// overwrites the slot, and the first result must not change under it.
#[rstest]
fn the_caller_owns_a_copy_not_a_view(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION MakePt : Point
        VAR_INPUT a : INT; b : INT; END_VAR
            MakePt.x := a;
            MakePt.y := b;
        END_FUNCTION

        FUNCTION run : INT
        VAR p : Point; q : Point; END_VAR
            p := MakePt(1, 2);
            q := MakePt(8, 9);
            run := p.x * 1000 + p.y * 100 + q.x * 10 + q.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1289, "p keeps 1,2 after the second call writes the slot");
}

/// An FB METHOD returning a struct — a different lowering path from a free
/// FUNCTION (it carries `this`, and its return slot is allocated in the
/// method-lowering loop), so the free-function tests prove nothing about it.
#[rstest]
fn fb_method_returning_a_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Holder
        VAR
            ox : INT := 10;
            oy : INT := 20;
        END_VAR
            METHOD PUBLIC GetPt : Point
            VAR tmp : Point; END_VAR
                tmp.x := ox;
                tmp.y := oy;
                GetPt := tmp;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR h : Holder; p : Point; END_VAR
            p := h.GetPt();
            run := p.x * 100 + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1020, "x = 10, y = 20, through the method's slot");
}

/// The same method built COMPONENT-WISE on the method name — the
/// self-reference resolves through `MethodDecl`, not `Function`, so the
/// resolver's return-type rooting must cover both.
#[rstest]
fn fb_method_built_component_wise(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Holder
        VAR
            ox : INT := 3;
            oy : INT := 4;
        END_VAR
            METHOD PUBLIC GetPt : Point
                GetPt.x := ox;
                GetPt.y := oy;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR h : Holder; p : Point; END_VAR
            p := h.GetPt();
            run := p.x * 100 + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 304, "component-wise through the METHOD's name");
}

/// A CLASS method returning a struct — the third lowering path.
#[rstest]
fn class_method_returning_a_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        CLASS C
        VAR
            ox : INT := 7;
            oy : INT := 9;
        END_VAR
            METHOD PUBLIC GetPt : Point
            VAR tmp : Point; END_VAR
                tmp.x := ox;
                tmp.y := oy;
                GetPt := tmp;
            END_METHOD
        END_CLASS

        FUNCTION run : INT
        VAR c : C; p : Point; END_VAR
            p := c.GetPt();
            run := p.x * 100 + p.y;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 709);
}
