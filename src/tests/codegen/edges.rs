// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! An edge input reads as its edge in the block's own code: TRUE for the one
//! call where the input went from FALSE to TRUE (`R_EDGE`) or from TRUE to
//! FALSE (`F_EDGE`), as `R_TRIG` and `F_TRIG` count it, first call included.
//! Outside the block, the input reads as it was given. A call that leaves
//! the input out keeps its last value, which is then no edge.

use crate::tests::codegen::{TestPlc, compile_to_wasm, run, with_db};
use rstest::*;

/// Each call appends a digit: 1 when the block saw an edge.
#[rstest]
fn an_edge_input_reads_as_its_edge(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Rising
        VAR_INPUT E : BOOL R_EDGE; END_VAR
        VAR_OUTPUT Q : BOOL; END_VAR
            Q := E;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Falling
        VAR_INPUT E : BOOL F_EDGE; END_VAR
        VAR_OUTPUT Q : BOOL; END_VAR
            Q := E;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Digits
        VAR_INPUT seen : BOOL; END_VAR
        VAR_OUTPUT n : DINT; END_VAR
            n := n * 10;
            IF seen THEN n := n + 1; END_IF;
        END_FUNCTION_BLOCK

        FUNCTION run_rising : DINT
        VAR r : Rising; d : Digits; END_VAR
            r(E := TRUE);  d(seen := r.Q);
            r(E := TRUE);  d(seen := r.Q);
            r();           d(seen := r.Q);
            r(E := FALSE); d(seen := r.Q);
            r();           d(seen := r.Q);
            r(E := TRUE);  d(seen := r.Q);
            run_rising := d.n;
            IF NOT r.E THEN run_rising := -1; END_IF;
        END_FUNCTION

        FUNCTION run_falling : DINT
        VAR f : Falling; d : Digits; END_VAR
            f(E := FALSE); d(seen := f.Q);
            f(E := TRUE);  d(seen := f.Q);
            f(E := FALSE); d(seen := f.Q);
            f();           d(seen := f.Q);
            run_falling := d.n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let rising: i32 = crate::tests::codegen::execute_wasm(&wasm, "run_rising", ());
    assert_eq!(
        rising, 100001,
        "TRUE on the first call, held, left out, FALSE, left out, TRUE again; r.E reads TRUE outside"
    );
    let falling: i32 = crate::tests::codegen::execute_wasm(&wasm, "run_falling", ());
    assert_eq!(
        falling, 1010,
        "FALSE on the first call, TRUE, FALSE, left out"
    );
}

/// IEC 61131-3's own example: `Z` is TRUE on the call where `X` rose and `Y`
/// fell.
#[rstest]
fn the_standards_and_edge_example(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK AND_EDGE
        VAR_INPUT X : BOOL R_EDGE; Y : BOOL F_EDGE; END_VAR
        VAR_OUTPUT Z : BOOL; END_VAR
            Z := X AND Y;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR a : AND_EDGE; END_VAR
            a(X := FALSE, Y := TRUE);
            IF a.Z THEN run := run + 1; END_IF;
            a(X := TRUE, Y := FALSE);
            IF a.Z THEN run := run + 10; END_IF;
            a(X := TRUE, Y := FALSE);
            IF a.Z THEN run := run + 100; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(result, 10, "only the call where X rose and Y fell");
}

/// The edge is detected once per call, by the instance's own body: a base's
/// body reached through `SUPER()` sees it too, where detecting it a second
/// time would find the memory already updated. A method reads the edge of
/// the last call.
#[rstest]
fn an_inherited_edge_is_detected_once_per_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR_INPUT E : BOOL R_EDGE; END_VAR
        VAR_OUTPUT base_seen : DINT; END_VAR
        METHOD PUBLIC Seen : BOOL
            Seen := E;
        END_METHOD
            IF E THEN base_seen := base_seen + 1; END_IF;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        VAR_OUTPUT derived_seen : DINT; END_VAR
            SUPER();
            IF E THEN derived_seen := derived_seen + 1; END_IF;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR d : Derived; END_VAR
            d(E := TRUE);
            IF d.Seen() THEN run := 1000; END_IF;
            d(E := TRUE);
            IF d.Seen() THEN run := run + 100; END_IF;
            run := run + d.base_seen * 10 + d.derived_seen;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(
        result, 1011,
        "the method saw the first call's edge and not the second's, each body counted one"
    );
}

/// A PROGRAM's input wired to an address in the configuration is copied in
/// before every scan, and the body sees its edge: one count per press, however
/// many scans the input stays TRUE.
#[rstest]
fn a_program_input_detects_its_edge(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Counter
        VAR_INPUT pulse : BOOL R_EDGE; END_VAR
        VAR_OUTPUT count : UINT; END_VAR
            IF pulse THEN count := count + 1; END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL total AT %QW0 : UINT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Counter(pulse := %IX0.0, count => total);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    let total = |plc: &TestPlc| {
        u16::from_le_bytes(
            plc.read_located("%QW0").expect("total")[..2]
                .try_into()
                .unwrap(),
        )
    };
    plc.write_located("%IX0.0", &1i32.to_le_bytes())
        .expect("press");
    plc.run(3).expect("held for three scans");
    assert_eq!(total(&plc), 1, "one press");
    plc.write_located("%IX0.0", &0i32.to_le_bytes())
        .expect("release");
    plc.run(1).expect("released");
    plc.write_located("%IX0.0", &1i32.to_le_bytes())
        .expect("press again");
    plc.run(2).expect("held for two scans");
    assert_eq!(total(&plc), 2, "two presses");
}
