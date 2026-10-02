// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Where every variable starts: its type's defaults (an alias's, a STRUCT's
//! fields', an instance's members', an enum's first value, a subrange's lower
//! limit), then its declaration's own initializer, overlaid by store order.
//! One rule, for a FUNCTION's locals and outputs and result, a METHOD's, an
//! FB's or PROGRAM's VAR_TEMP, and the statics `__init` writes.

use crate::tests::codegen::{TestPlc, compile_to_mir_and_wasm, with_db};
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// An instance initializer names some members; the others keep their own
/// defaults, as a STRUCT's fields do. They used to start at 0.
#[rstest]
fn a_partial_instance_initializer_keeps_the_other_defaults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
        VAR_OUTPUT speed : INT := 5; limit : INT := 6; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Fast EXTENDS Motor
        VAR_OUTPUT boost : INT := 7; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR_OUTPUT m : Motor := (speed := 9); END_VAR
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            m : Motor := (speed := 9);
            f : Fast := (boost := 1);
            h : Holder;
            a : ARRAY[0..1] OF Motor := [(speed := 1), (speed := 2)];
        END_VAR
            IF m.speed = 9 THEN test := 20000; END_IF;
            test := test + m.limit * 1000 + f.speed * 100 + h.m.limit * 10 + a[1].limit;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 26566, "9 named, then 6, 5, 6 and 6 kept");
}

/// IEC 61131-3: an enumerated value starts at its first value, in
/// declaration order, and a subrange at its lower limit. Both used to start
/// at 0, which neither of these types has, and passing the subrange on
/// trapped.
#[rstest]
fn enums_and_subranges_start_inside_their_type(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Step : (A := 5, B, C);
            Holder : STRUCT s : Step; END_STRUCT;
            Pct : INT(5..10);
            Order : (Hi := 5, Lo := 1);
            Signed : INT(-5..5);
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR_OUTPUT s : Step; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION Echo : Pct
        VAR_INPUT p : Pct; END_VAR
            Echo := p;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            s : Step;
            h : Holder;
            a : ARRAY[0..1] OF Step;
            f : Fb;
            p : Pct;
            o : Order;
            n : Signed;
        END_VAR
            IF s = Step#A THEN test := test + 1; END_IF;
            IF h.s = Step#A THEN test := test + 1; END_IF;
            IF a[1] = Step#A THEN test := test + 1; END_IF;
            IF f.s = Step#A THEN test := test + 1; END_IF;
            IF o = Order#Hi THEN test := test + 1; END_IF;
            test := test + Echo(p) * 10;
            IF n = -5 THEN test := test + 100; END_IF;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        r, 155,
        "four at Step#A, Order#Hi, Pct at 5 and Signed at -5"
    );
}

/// A VAR_EXTERNAL is the global's storage and a VAR_IN_OUT the caller's:
/// neither starts over. The external's type defaults were written at every
/// call anyway, from address 0 on, over the caller's locals.
#[rstest]
fn a_var_external_or_in_out_writes_nothing_at_entry(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Big : ARRAY[0..4999] OF DINT := [5000(-1)];
            Small : ARRAY[0..9] OF DINT := [10(-1)];
        END_TYPE

        FUNCTION Touch : DINT
        VAR_EXTERNAL gb : Big; END_VAR
            Touch := gb[0];
        END_FUNCTION

        FUNCTION Locals : DINT
        VAR s : ARRAY[0..9] OF DINT; d : DINT; END_VAR
            s[0] := 42;
            s[9] := 43;
            d := Touch();
            Locals := s[0] * 100 + s[9];
        END_FUNCTION

        FUNCTION Keep : DINT
        VAR_IN_OUT b : Small; END_VAR
            Keep := b[3];
            b[4] := 7;
        END_FUNCTION

        PROGRAM P
        VAR
            fromGlobal : DINT;
            frame : DINT;
            kept : DINT;
            written : DINT;
            mine : Small;
        END_VAR
            fromGlobal := Touch();
            frame := Locals();
            mine[3] := 42;
            kept := Keep(b := mine);
            written := mine[4];
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL gb : Big; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    let dbg = DebugInfo::from_wasm(&wasm);
    plc.run(1).expect("scan");
    let read = |path| plc.read_var(&dbg, path);
    assert_eq!(
        read("P1.fromGlobal"),
        Some(VarValue::I32(-1)),
        "__init wrote the global"
    );
    assert_eq!(
        read("P1.frame"),
        Some(VarValue::I32(4243)),
        "the caller's array is untouched"
    );
    assert_eq!(
        read("P1.kept"),
        Some(VarValue::I32(42)),
        "the caller's element"
    );
    assert_eq!(read("P1.written"), Some(VarValue::I32(7)));
}

/// A FUNCTION starts over at every call: its VAR_OUTPUT from its initializer,
/// written into the caller's variable, and its result from its type's
/// defaults. Each body below changes what it started with, and runs twice.
#[rstest]
fn outputs_and_results_start_over_at_every_call(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Step : (A := 5, B);
            Pt : STRUCT x : INT := 3; y : INT; END_STRUCT;
        END_TYPE

        FUNCTION WithOutput : INT
        VAR_OUTPUT o : INT := 7; END_VAR
            o := o + 1;
            WithOutput := 1;
        END_FUNCTION

        FUNCTION StepResult : Step
        END_FUNCTION

        FUNCTION PtResult : Pt
            PtResult.x := PtResult.x + 1;
            PtResult.y := PtResult.y + 4;
        END_FUNCTION

        FUNCTION_BLOCK Fb
            METHOD PUBLIC Out : INT
            VAR_OUTPUT o : INT := 8; END_VAR
                o := o + 1;
                Out := 1;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR r : INT := 99; q : INT := 99; one : INT; p : Pt; f : Fb; END_VAR
            one := WithOutput(o => r);
            one := WithOutput(o => r);
            one := f.Out(o => q);
            one := f.Out(o => q);
            p := PtResult();
            p := PtResult();
            IF StepResult() = Step#A THEN test := 1; END_IF;
            IF p.y = 4 THEN test := test + 20000; END_IF;
            test := test + r * 10 + q * 100 + p.x * 1000;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        r, 24981,
        "Step#A, o = 7 + 1, METHOD o = 8 + 1, x = 3 + 1 from Pt, y = 4"
    );
}

/// An output without an initializer starts over as well, from its type's
/// defaults: 0, the first value, a STRUCT's fields, the empty STRING. It used
/// to keep what the caller's variable held.
#[rstest]
fn an_output_without_initializer_starts_over_too(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Step : (A := 5, B);
            Pt : STRUCT x : INT := 3; y : INT; END_STRUCT;
        END_TYPE

        FUNCTION Plain : INT
        VAR_OUTPUT n : INT; s : Step; p : Pt; t : STRING; END_VAR
            n := n + 1;
            IF s = Step#A THEN n := n + 10; END_IF;
            IF t = '' THEN n := n + 100; END_IF;
            p.x := p.x + 1;
            p.y := p.y + 1;
            s := Step#B;
            t := 'x';
            Plain := 1;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            n : INT := 99;
            s : Step := Step#B;
            p : Pt := (x := 50, y := 60);
            t : STRING := 'abc';
            one : INT;
        END_VAR
            one := Plain(n => n, s => s, p => p, t => t);
            one := Plain(n => n, s => s, p => p, t => t);
            test := n * 100 + p.x * 10 + p.y;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 11141, "n = 1 + 10 + 100, p = (3 + 1, 0 + 1), twice over");
}

/// An FB's VAR_TEMP starts over at every call from its type's defaults, as a
/// FUNCTION's locals do (a VAR_TEMP takes no initializer of its own, E0004).
#[rstest]
fn var_temp_starts_over_at_every_call(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Pt : STRUCT x : INT := 3; END_STRUCT;
            Step : (A := 5, B);
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR_OUTPUT seen : INT; END_VAR
        VAR_TEMP
            p : Pt;
            s : Step;
        END_VAR
            p.x := p.x + 1;
            seen := p.x;
            IF s = Step#A THEN seen := seen + 10; END_IF;
            s := Step#B;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : Fb; END_VAR
            f();
            f();
            test := f.seen;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 14, "x at 3 and s at Step#A again on the second call");
}

/// An instance held by a STRUCT field starts from its members' defaults too.
#[rstest]
fn an_instance_in_a_struct_gets_its_member_defaults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_OUTPUT n : INT := 4; END_VAR
        END_FUNCTION_BLOCK

        TYPE H : STRUCT c : Counter; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR h : H; END_VAR
            test := h.c.n;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 4);
}

/// The same rule for the statics `__init` writes, once, and a PROGRAM's
/// VAR_TEMP at every scan.
#[rstest]
fn a_program_starts_the_same_way(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Step : (A := 5, B);
            Pct : INT(5..10);
            Pt : STRUCT x : INT := 3; END_STRUCT;
            Count : INT := 3;
        END_TYPE

        FUNCTION_BLOCK Motor
        VAR_OUTPUT speed : INT := 5; limit : INT := 6; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            s : Step;
            p : Pct;
            m : Motor := (speed := 9);
            atA : INT;
            pv : INT;
            limit : INT;
            tt : INT;
            c : Count;
        END_VAR
        VAR_TEMP t : Pt; END_VAR
            IF s = Step#A THEN atA := 1; END_IF;
            pv := p;
            limit := m.limit;
            t.x := t.x + 1;
            tt := t.x;
            c := c + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Run WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    let dbg = DebugInfo::from_wasm(&wasm);
    plc.run(2).expect("scans");
    assert_eq!(plc.read_var(&dbg, "Run.atA"), Some(VarValue::I16(1)));
    assert_eq!(plc.read_var(&dbg, "Run.pv"), Some(VarValue::I16(5)));
    assert_eq!(plc.read_var(&dbg, "Run.limit"), Some(VarValue::I16(6)));
    assert_eq!(
        plc.read_var(&dbg, "Run.tt"),
        Some(VarValue::I16(4)),
        "3 + 1, at every scan"
    );
    assert_eq!(
        plc.read_var(&dbg, "Run.c"),
        Some(VarValue::I16(5)),
        "3 + 1 + 1, from one start"
    );
}
