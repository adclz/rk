// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `x += e` computes `x := x + e`, and `-=`, `*=`, `/=` alike, with the
//! place `x` found once for the read and the store.

use super::with_db;
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

/// Each operator computes what its operation written out does, at the
/// place's type: an integer division truncates, an integer wraps at its
/// width, an operand of a narrower type widens, and REALs and durations
/// compute as theirs.
#[rstest]
fn each_operator_computes_its_operation(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION integers
        VAR i : INT := 10; END_VAR
            i += 5; IF i <> 15 THEN __RAISE('+='); END_IF;
            i -= 20; IF i <> -5 THEN __RAISE('-='); END_IF;
            i *= -3; IF i <> 15 THEN __RAISE('*='); END_IF;
            i /= 4; IF i <> 3 THEN __RAISE('/= truncates'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION wraps_at_the_width
        VAR i : INT := 32767; s : SINT := -128; u : USINT := 0; END_VAR
            i += 1; IF i <> -32768 THEN __RAISE('INT wraps'); END_IF;
            s -= 1; IF s <> 127 THEN __RAISE('SINT wraps'); END_IF;
            u -= 1; IF u <> 255 THEN __RAISE('USINT wraps'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION widened_operands
        VAR d : DINT := 100000; i : INT := 7; l : LREAL := 1.0; END_VAR
            d += i; IF d <> 100007 THEN __RAISE('DINT += INT'); END_IF;
            l += i; IF l <> 8.0 THEN __RAISE('LREAL += INT'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION reals_and_durations
        VAR r : REAL := 1.5; t : TIME := T#1s; END_VAR
            r *= 2.0; r += 0.5; r /= 0.5; IF r <> 7.0 THEN __RAISE('REAL'); END_IF;
            t += T#500ms; IF t <> T#1500ms THEN __RAISE('TIME +='); END_IF;
            t -= T#2s; IF t <> T#-500ms THEN __RAISE('TIME -='); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 4);
}

/// Any place an assignment writes: an element, a field, a dereference, an
/// input bound by reference, an instance's member from outside and from
/// inside, the result.
#[rstest]
fn every_place_an_assignment_writes(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : REAL; END_STRUCT; END_TYPE
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION_BLOCK Counter
        VAR_INPUT step : INT; END_VAR
        VAR count : INT; END_VAR
            count += step;
        END_FUNCTION_BLOCK

        FUNCTION twice : INT
        VAR_IN_OUT n : INT; END_VAR
            n *= 2;
            twice := n;
        END_FUNCTION

        FUNCTION acc : INT
        VAR_INPUT a : INT; END_VAR
            acc := a;
            acc += 10;
            acc *= 2;
        END_FUNCTION

        {test}
        FUNCTION places
        VAR arr : ARRAY[0..5] OF INT; p : Pt; x : INT := 3; r : PInt; n : INT := 5; c : Counter; END_VAR
            arr[2] := 4; arr[2] += 1; IF arr[2] <> 5 THEN __RAISE('element'); END_IF;
            p.x := 1; p.x -= 3; IF p.x <> -2 THEN __RAISE('field'); END_IF;
            p.y += 2.5; IF p.y <> 2.5 THEN __RAISE('REAL field'); END_IF;
            r := REF(x); r^ *= 7; IF x <> 21 THEN __RAISE('dereference'); END_IF;
            IF twice(n := n) <> 10 OR n <> 10 THEN __RAISE('in-out'); END_IF;
            c.count += 2; c(step := 3); IF c.count <> 5 THEN __RAISE('member'); END_IF;
            IF acc(a := 1) <> 22 THEN __RAISE('result'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 1);
}

/// The place is found once for the read and the store: a call in its
/// subscript runs once, and a reference the value moves still has the
/// place it pointed at when the statement began.
#[rstest]
fn the_place_is_found_once(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION idx : INT
        VAR_IN_OUT calls : INT; END_VAR
            calls += 1;
            idx := 2;
        END_FUNCTION

        FUNCTION aim : INT
        VAR_IN_OUT r : PInt; target : INT; END_VAR
            r := REF(target);
            aim := 1;
        END_FUNCTION

        {test}
        FUNCTION a_call_in_the_subscript_runs_once
        VAR arr : ARRAY[0..5] OF INT; n : INT; END_VAR
            arr[2] := 40;
            arr[idx(calls := n)] += 2;
            IF n <> 1 OR arr[2] <> 42 THEN __RAISE('one call'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_reference_moved_by_the_value
        VAR x : INT := 10; y : INT := 100; r : PInt; END_VAR
            r := REF(x);
            r^ += aim(r := r, target := y);
            IF x <> 11 OR y <> 100 THEN __RAISE('x read and written'); END_IF;
        END_FUNCTION
    "#;
    all_pass(&mut with_db, source, 2);
}

/// A subrange checks what the store puts in it: 8 + 5 stops at 0..10.
#[rstest]
fn a_subrange_checks_the_store(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION in_range
        VAR sr : INT(0..10) := 8; END_VAR
            sr += 2;
            IF sr <> 10 THEN __RAISE('10'); END_IF;
        END_FUNCTION

        {test}
        FUNCTION out_of_range
        VAR sr : INT(0..10) := 8; END_VAR
            sr += 5;
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
