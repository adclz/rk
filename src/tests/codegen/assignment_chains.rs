// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `a := b := e` runs from right to left, as `b := e; a := b;` does with `b`
//! read where it was stored: the last place is written the value, then each
//! other the value of the place after it, and each place is reached once.

use super::{TestPlc, compile_to_mir_and_wasm, with_db};
use rstest::*;

/// Compiles `source` and runs its `{test}` functions: each must pass.
fn all_pass(db: &mut db::RootDatabase, source: &str, count: usize) {
    let wasm = super::compile_to_wasm_as_built(db, source);
    let results = super::run_tests(&wasm, None).expect("run tests");
    assert_eq!(results.len(), count);
    for result in &results {
        assert!(result.passed(), "{}: {:?}", result.name, result.reason);
    }
}

/// Every place of a chain holds the value: scalars, a named value, a
/// structure copied, a reference, the FUNCTION's result.
#[rstest]
fn every_place_takes_the_value(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
        TYPE Mode : (Idle, Run, Stop); END_TYPE

        FUNCTION five : INT
        VAR x : INT; END_VAR
            five := x := 5;
        END_FUNCTION

        FUNCTION seven : INT
        VAR_OUTPUT x : INT; END_VAR
            x := seven := 7;
        END_FUNCTION

        {test}
        FUNCTION scalars
        VAR a, b, c : INT; on, ready : BOOL; END_VAR
            a := b := c := 7;
            IF a <> 7 OR b <> 7 OR c <> 7 THEN __RAISE('7 7 7'); END_IF;
            on := ready := TRUE;
            IF NOT on OR NOT ready THEN __RAISE('TRUE TRUE'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_named_value
        VAR m1, m2 : Mode; END_VAR
            m1 := m2 := Mode#Run;
            IF m1 <> Mode#Run OR m2 <> Mode#Run THEN __RAISE('Run Run'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_structure
        VAR p1, p2, p3 : Pt; END_VAR
            p3.x := 1;
            p3.y := 2;
            p1 := p2 := p3;
            IF p1.x <> 1 OR p1.y <> 2 OR p2.x <> 1 OR p2.y <> 2 THEN
                __RAISE('copied twice');
            END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_reference
        VAR x : INT; r1, r2 : REF_TO INT; END_VAR
            r2 := r1 := REF(x);
            r2^ := 5;
            IF x <> 5 OR r1^ <> 5 THEN __RAISE('both point at x'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION the_result
        VAR out : INT; END_VAR
            IF five() <> 5 THEN __RAISE('the result is 5'); END_IF;
            IF seven(x => out) <> 7 OR out <> 7 THEN __RAISE('7 read back'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 5);
}

/// Each place takes the value the place to its right holds: converted at
/// each link, and cut to a STRING's capacity before the next takes it.
#[rstest]
fn each_place_takes_the_value_of_the_place_to_its_right(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION widened_at_each_link
        VAR a : INT; d : DINT; r : LREAL; END_VAR
            r := d := a := -5;
            IF a <> -5 OR d <> -5 OR r <> -5.0 THEN __RAISE('-5 at each'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION cut_to_the_capacity
        VAR s4 : STRING[4]; s80 : STRING; text : STRING := 'abcdefgh'; END_VAR
            s80 := s4 := text;
            IF s4 <> 'abcd' OR s80 <> 'abcd' THEN __RAISE('abcd in both'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 2);
}

/// The last link runs first, and a place is found when its link runs:
/// `arr[i] := i := 3` writes `arr[3]`.
#[rstest]
fn a_chain_runs_from_right_to_left(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION a_subscript_written_first
        VAR arr : ARRAY[0..5] OF INT; i : INT := 1; END_VAR
            arr[i] := i := 3;
            IF arr[3] <> 3 OR arr[1] <> 0 OR i <> 3 THEN __RAISE('arr[3]'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_subscript_written_last
        VAR arr : ARRAY[0..5] OF INT; i : INT := 1; END_VAR
            i := arr[i] := 5;
            IF arr[1] <> 5 OR i <> 5 THEN __RAISE('arr[1]'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 2);
}

/// A call in a subscript runs once, and a place read back by the next link
/// is read where its value was stored, though that link's own place moved
/// what its subscript or its reference reads.
#[rstest]
fn a_place_is_reached_once(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION aim : INT
        VAR_IN_OUT r : PInt; target : INT; END_VAR
            r := REF(target);
            aim := 2;
        END_FUNCTION

        FUNCTION idx : INT
        VAR_IN_OUT calls : INT; END_VAR
            calls := calls + 1;
            idx := 2;
        END_FUNCTION

        FUNCTION bump : INT
        VAR_IN_OUT k : INT; END_VAR
            k := k + 1;
            bump := k;
        END_FUNCTION

        {test}
        FUNCTION a_call_in_a_place_read_back
        VAR arr : ARRAY[0..5] OF INT; a, n : INT; END_VAR
            a := arr[idx(calls := n)] := 9;
            IF n <> 1 OR arr[2] <> 9 OR a <> 9 THEN __RAISE('one call'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_call_in_the_first_place
        VAR arr : ARRAY[0..5] OF INT; a, n : INT; END_VAR
            arr[idx(calls := n)] := a := 4;
            IF n <> 1 OR arr[2] <> 4 OR a <> 4 THEN __RAISE('one call'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION read_back_where_stored
        VAR arr, arr2 : ARRAY[0..5] OF INT; k : INT := 1; END_VAR
            arr[bump(k := k)] := arr2[k] := 5;
            IF arr2[1] <> 5 OR arr[2] <> 5 OR k <> 2 THEN __RAISE('arr2[1] read'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION read_back_through_a_moved_reference
        VAR arr : ARRAY[0..5] OF INT; x, y : INT; r : PInt; END_VAR
            r := REF(x);
            arr[aim(r := r, target := y)] := r^ := 5;
            IF x <> 5 OR y <> 0 OR arr[2] <> 5 THEN __RAISE('x read back'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 4);
}

/// A bit of a word in a chain is stored into the word and read back from it.
#[rstest]
fn a_bit_in_a_chain(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION a_bit_read_back
        VAR w : WORD; b : BOOL; END_VAR
            b := w.%X3 := TRUE;
            IF w <> 8 OR NOT b THEN __RAISE('bit 3'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_bit_written_last
        VAR w : WORD; c : BOOL; END_VAR
            w.%X5 := c := TRUE;
            IF w <> 32 OR NOT c THEN __RAISE('bit 5'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 2);
}

/// A located bit inside a located word is a view of the word: a chain stores
/// through it and reads it back from the word.
#[rstest]
fn a_view_in_a_chain(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL bit3 : BOOL; seen : BOOL; END_VAR
            seen := bit3 := TRUE;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            word AT %QW0 : WORD;
            bit3 AT %QX0.3 : BOOL;
            seen AT %QX2.0 : BOOL;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(plc.read_located("%QW0").expect("word")[0], 8);
    assert_eq!(plc.read_located("%QX2.0").expect("seen")[0] & 1, 1);
}

/// A subrange checks the value its link stores: 20 passes through an INT and
/// stops at the subrange.
#[rstest]
fn a_link_checks_its_subrange(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION in_range
        VAR sr : INT(0..10); i : INT; END_VAR
            i := sr := 5;
            IF i <> 5 OR sr <> 5 THEN __RAISE('5 in both'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION out_of_range
        VAR sr : INT(0..10); i : INT; END_VAR
            sr := i := 20;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm_as_built(&mut with_db, source);
    let results = super::run_tests(&wasm, None).expect("run tests");
    let outcome = |name: &str| {
        results
            .iter()
            .find(|result| result.name.ends_with(name))
            .unwrap_or_else(|| panic!("{name} ran"))
    };
    assert!(outcome("in_range").passed());
    let out = outcome("out_of_range");
    assert!(!out.passed());
    assert!(
        out.reason
            .as_deref()
            .is_some_and(|reason| reason.contains("subrange")),
        "{:?}",
        out.reason
    );
}
