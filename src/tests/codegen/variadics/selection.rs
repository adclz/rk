// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `MIN`, `MAX`, `LIMIT` and `SEL` on each type they take. These are here and not
//! among the library's own `{test}`s, which are checked at each `rk check` of a
//! workspace that reads it.

use db::RootDatabase;
use rstest::rstest;

use super::mux::assert_holds;
use super::{ordered, with_db};

#[rstest]
#[case::bool("BOOL")]
#[case::sint("SINT")]
#[case::int("INT")]
#[case::dint("DINT")]
#[case::lint("LINT")]
#[case::usint("USINT")]
#[case::uint("UINT")]
#[case::udint("UDINT")]
#[case::ulint("ULINT")]
#[case::byte("BYTE")]
#[case::word("WORD")]
#[case::dword("DWORD")]
#[case::lword("LWORD")]
#[case::real("REAL")]
#[case::lreal("LREAL")]
#[case::time("TIME")]
#[case::ltime("LTIME")]
#[case::date("DATE")]
#[case::ldate("LDATE")]
#[case::tod("TOD")]
#[case::ltod("LTOD")]
#[case::dt("DT")]
#[case::ldt("LDT")]
#[case::char("CHAR")]
fn on_each_type(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let [a, b, c] = ordered(ty);
    assert_holds(
        &mut with_db,
        "",
        "",
        &[
            format!("MIN({b}) = {b}"),
            format!("MIN({a}, {b}) = {a}"),
            format!("MIN({b}, {a}) = {a}"),
            format!("MIN({c}, {a}, {b}) = {a}"),
            format!("MAX({b}) = {b}"),
            format!("MAX({a}, {b}) = {b}"),
            format!("MAX({b}, {a}) = {b}"),
            format!("MAX({a}, {c}, {b}) = {c}"),
            format!("LIMIT({a}, {b}, {c}) = {b}"),
            format!("LIMIT({b}, {a}, {c}) = {b}"),
            format!("LIMIT({a}, {c}, {b}) = {b}"),
            format!("LIMIT(MN := {a}, IN := {b}, MX := {c}) = {b}"),
            // MX wins when MN is above it.
            format!("LIMIT({c}, {b}, {a}) = {a}"),
            format!("SEL(FALSE, {a}, {b}) = {a}"),
            format!("SEL(TRUE, {a}, {b}) = {b}"),
            format!("SEL(G := TRUE, IN0 := {a}, IN1 := {b}) = {b}"),
        ],
    );
}

/// A STRING takes two, three or four inputs, and compares as an operator does: a
/// shorter one is less than a longer one it starts.
#[rstest]
fn on_strings(mut with_db: RootDatabase) {
    let holds = [
        "MIN('a', 'b') = 'a'",
        "MIN(s, 'c') = 'b'",
        "MIN('c', 'a', 'b') = 'a'",
        "MIN('d', 'c', 'a', 'b') = 'a'",
        "MIN('ab', 'a') = 'a'",
        "MIN(IN1 := 'a', IN2 := 'b') = 'a'",
        "MAX('a', 'b') = 'b'",
        "MAX(s, 'a') = 'b'",
        "MAX('a', 'c', 'b') = 'c'",
        "MAX('a', 'd', 'c', 'b') = 'd'",
        "MAX('ab', 'a') = 'ab'",
        "MAX(IN1 := 'a', IN2 := 'b') = 'b'",
        "LIMIT('b', 'a', 'c') = 'b'",
        "LIMIT('b', 'z', 'c') = 'c'",
        "LIMIT('a', s, 'c') = 'b'",
        "LIMIT('c', 'b', 'a') = 'a'",
        "SEL(FALSE, 'a', 'b') = 'a'",
        "SEL(TRUE, s, 'a') = 'a'",
    ]
    .map(str::to_string);
    assert_holds(&mut with_db, "", "s : STRING := 'b';", &holds);
}

/// Inputs of two types land on the wider one, and keep their value there.
#[rstest]
fn inputs_of_two_types_widen(mut with_db: RootDatabase) {
    let holds = [
        "MIN(si, n) = -1",
        "MAX(si, n) = 200",
        "MAX(flag, BYTE#0) = BYTE#1",
        "MIN(BYTE#255, WORD#256) = WORD#255",
        "MIN(T#1s, LT#2s) = LT#1s",
        "MAX(D#2020-01-01, LD#2020-01-02) = LD#2020-01-02",
        "MIN(TOD#01:00:00, LTOD#02:00:00) = LTOD#01:00:00",
        "MAX(DT#2020-01-01-01:00:00, LDT#2020-01-02-01:00:00) = LDT#2020-01-02-01:00:00",
        "LIMIT(T#1s, LT#2s, T#3s) = LT#2s",
        "SEL(TRUE, T#1s, LT#2s) = LT#2s",
    ]
    .map(str::to_string);
    assert_holds(
        &mut with_db,
        "",
        "si : SINT := -1; n : INT := 200; flag : BOOL := TRUE;",
        &holds,
    );
}

/// Literals alone pick an overload as they do in an expression: an integer is
/// INT, a real is REAL, and each kind of literal lands on its own type.
#[rstest]
fn bare_literals_pick_an_overload(mut with_db: RootDatabase) {
    let holds = [
        "MIN(1, 2) = 1",
        "MAX(1, 2, 3) = 3",
        "LIMIT(0, 5, 10) = 5",
        "SEL(TRUE, 1, 2) = 2",
        "MIN(1.5, 2.5) = 1.5",
        "MAX(1.5, 2.5, 0.5) = 2.5",
        "MIN(TRUE, FALSE) = FALSE",
        "MAX(TRUE, FALSE) = TRUE",
        "MIN(T#1s, T#2s) = T#1s",
        "LIMIT(T#1s, T#5s, T#3s) = T#3s",
        "SEL(TRUE, T#1s, T#2s) = T#2s",
        "text = 'a'",
    ]
    .map(str::to_string);
    assert_holds(&mut with_db, "", "text : STRING := MIN('a', 'b');", &holds);
}
