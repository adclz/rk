// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `Std.Comparison` on each type it takes. These are here and not among the
//! library's own `{test}`s, which are checked at each `rk check` of a workspace
//! that reads it.

use rstest::rstest;

use crate::tests::codegen::with_db;

const COMPARISON: &str = include_str!("../../../stdlib/Comparison.st");

/// Each function, TRUE and FALSE, at the edges of the type where it has edges, so
/// that an unsigned type compared as signed fails. Each wrong call sets a bit of
/// its own.
#[rstest]
#[case::sint("SINT#-128", "SINT#0", "SINT#127")]
#[case::int("INT#-32768", "INT#0", "INT#32767")]
#[case::dint("DINT#-2147483648", "DINT#0", "DINT#2147483647")]
#[case::lint("LINT#-9223372036854775808", "LINT#0", "LINT#9223372036854775807")]
#[case::usint("USINT#0", "USINT#100", "USINT#200")]
#[case::uint("UINT#0", "UINT#100", "UINT#60000")]
#[case::udint("UDINT#0", "UDINT#100", "UDINT#4000000000")]
#[case::ulint("ULINT#0", "ULINT#100", "ULINT#18000000000000000000")]
#[case::real("REAL#-1.5", "REAL#0.5", "REAL#2.5")]
#[case::lreal("LREAL#-1.5", "LREAL#0.5", "LREAL#2.5")]
#[case::time("T#-1s", "T#0s", "T#1s")]
#[case::ltime("LT#-1s", "LT#0s", "LT#1s")]
#[case::byte("BYTE#0", "BYTE#100", "BYTE#255")]
#[case::word("WORD#0", "WORD#100", "WORD#65535")]
#[case::dword("DWORD#0", "DWORD#100", "DWORD#4294967295")]
#[case::lword("LWORD#0", "LWORD#100", "LWORD#18446744073709551615")]
#[case::date("D#2020-01-01", "D#2020-01-02", "D#2020-01-03")]
#[case::ldate("LD#2020-01-01", "LD#2020-01-02", "LD#2020-01-03")]
#[case::tod("TOD#01:00:00", "TOD#02:00:00", "TOD#03:00:00")]
#[case::ltod("LTOD#01:00:00", "LTOD#02:00:00", "LTOD#03:00:00")]
#[case::dt(
    "DT#2020-01-01-01:00:00",
    "DT#2020-01-02-01:00:00",
    "DT#2020-01-03-01:00:00"
)]
#[case::ldt(
    "LDT#2020-01-01-01:00:00",
    "LDT#2020-01-02-01:00:00",
    "LDT#2020-01-03-01:00:00"
)]
#[case::char("CHAR#'a'", "CHAR#'b'", "CHAR#'c'")]
fn each_function_on_each_type(
    mut with_db: db::RootDatabase,
    #[case] low: &str,
    #[case] mid: &str,
    #[case] high: &str,
) {
    let calls = [
        format!("GT({high}, {low})"),
        format!("GT({high}, {mid}, {low})"),
        format!("NOT GT({low}, {high})"),
        format!("NOT GT({high}, {high})"),
        format!("NOT GT({high}, {low}, {mid})"),
        format!("GE({high}, {high}, {low})"),
        format!("GE({high}, {mid}, {mid}, {low})"),
        format!("NOT GE({low}, {high})"),
        format!("NOT GE({high}, {low}, {mid})"),
        format!("EQ({mid}, {mid})"),
        format!("EQ({mid}, {mid}, {mid})"),
        format!("NOT EQ({mid}, {high})"),
        format!("NOT EQ({mid}, {mid}, {high})"),
        format!("NOT EQ({low}, {mid}, {mid})"),
        format!("LE({low}, {low}, {high})"),
        format!("LE({low}, {mid}, {mid}, {high})"),
        format!("NOT LE({high}, {low})"),
        format!("NOT LE({low}, {high}, {mid})"),
        format!("LT({low}, {high})"),
        format!("LT({low}, {mid}, {high})"),
        format!("NOT LT({high}, {low})"),
        format!("NOT LT({low}, {low})"),
        format!("NOT LT({low}, {high}, {mid})"),
        format!("NE({low}, {high})"),
        format!("NOT NE({mid}, {mid})"),
        format!("NE(IN1 := {low}, IN2 := {high})"),
    ];
    assert_each_holds(&mut with_db, &calls);
}

/// Each call answers TRUE, in a program of its own that reports the first which
/// does not.
fn assert_each_holds(db: &mut db::RootDatabase, calls: &[String]) {
    let mut source = String::from("USING Std.Comparison;\nFUNCTION test : DINT\n");
    for (i, call) in calls.iter().enumerate() {
        source += &format!(
            "    IF NOT ({call}) THEN test := {}; RETURN; END_IF;\n",
            i + 1
        );
    }
    source += "END_FUNCTION\n";

    let wasm = crate::tests::codegen::compile_with_libraries(db, &[COMPARISON], &source);
    let failed: i32 = super::execute_wasm(&wasm, "test", ());
    if failed != 0 {
        panic!("{} is false", calls[failed as usize - 1]);
    }
}

/// A BOOL has two values, and FALSE is the smaller.
#[rstest]
fn on_bool(mut with_db: db::RootDatabase) {
    let calls = [
        "GT(TRUE, FALSE)",
        "NOT GT(FALSE, TRUE)",
        "NOT GT(TRUE, TRUE, FALSE)",
        "GE(TRUE, TRUE, FALSE)",
        "NOT GE(FALSE, TRUE)",
        "EQ(TRUE, TRUE, TRUE)",
        "NOT EQ(TRUE, TRUE, FALSE)",
        "LE(FALSE, FALSE, TRUE)",
        "NOT LE(TRUE, FALSE)",
        "LT(FALSE, TRUE)",
        "NOT LT(FALSE, TRUE, TRUE)",
        "NE(TRUE, FALSE)",
        "NOT NE(TRUE, TRUE)",
    ]
    .map(str::to_string);
    assert_each_holds(&mut with_db, &calls);
}

/// A STRING takes two, three or four inputs, each named too, and compares as an
/// operator does: a shorter one is less than a longer one it starts.
#[rstest]
fn on_strings(mut with_db: db::RootDatabase) {
    let calls = [
        "GT('b', 'a')",
        "GT('c', 'b', 'a')",
        "GT('d', 'c', 'b', 'a')",
        "NOT GT('d', 'c', 'c', 'a')",
        "GT('ab', 'a')",
        "GT(IN1 := 'b', IN2 := 'a')",
        "GE('b', 'b')",
        "GE('c', 'b', 'b')",
        "GE('d', 'c', 'c', 'a')",
        "NOT GE('a', 'b', 'b')",
        "EQ('a', 'a')",
        "EQ('a', 'a', 'a')",
        "EQ('a', 'a', 'a', 'a')",
        "NOT EQ('a', 'a', 'a', 'b')",
        "LE('a', 'a')",
        "LE('a', 'a', 'b')",
        "LE('a', 'b', 'b', 'c')",
        "NOT LE('b', 'a', 'a')",
        "LT('a', 'b')",
        "LT('a', 'b', 'c')",
        "LT('a', 'b', 'c', 'd')",
        "NOT LT('a', 'b', 'b', 'd')",
        "NE('a', 'b')",
        "NOT NE('a', 'a')",
        "NE(IN1 := 'a', IN2 := 'b')",
    ]
    .map(str::to_string);
    assert_each_holds(&mut with_db, &calls);
}

/// How the calls pick an overload: literals, and two types landing on the wider
/// one. An input of a narrower type widens before it is compared, so an INT of
/// -1 is less than a DINT of 70000 whatever its bits.
#[rstest]
fn arguments_pick_the_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        USING Std.Comparison;

        FUNCTION test : DINT
        VAR i : INT := -1; d : DINT := 70000; u : UINT := 60000; r : REAL := 2.5; END_VAR
            IF NOT LT(1, 2) THEN test := test + 1; END_IF;
            IF NOT EQ(1.5, 1.5) THEN test := test + 2; END_IF;
            IF NOT GT(T#2s, T#1s) THEN test := test + 4; END_IF;
            IF NOT LT(i, d) THEN test := test + 8; END_IF;
            IF NOT LE(i, r, d) THEN test := test + 16; END_IF;
            IF NOT LT(u, d) THEN test := test + 32; END_IF;
            IF NOT NE(i, d) THEN test := test + 64; END_IF;
            IF NOT GE(LT#2s, T#1s) THEN test := test + 128; END_IF;
        END_FUNCTION
    "#;
    let wasm = crate::tests::codegen::compile_with_libraries(&mut with_db, &[COMPARISON], source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 0, "each wrong call sets its own bit");
}

/// The names stay free: a variable named `LT` hides the function, a FUNCTION of
/// the program named `EQ` is the one called, and `LT#1s` is still a literal.
#[rstest]
fn the_names_stay_free(mut with_db: db::RootDatabase) {
    let source = r#"
        USING Std.Comparison;

        FUNCTION EQ : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            EQ := a + b;
        END_FUNCTION

        FUNCTION test : DINT
        VAR LT : INT := 3; t : LTIME := LT#1s; END_VAR
            test := EQ(1, 2) * 10 + LT;
            IF GE(t, LT#1s) THEN
                test := test + 100;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = crate::tests::codegen::compile_with_libraries(&mut with_db, &[COMPARISON], source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 133);
}
