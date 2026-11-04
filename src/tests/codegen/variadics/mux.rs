// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `MUX` on each type it takes. These are here and not among the library's own
//! `{test}`s, which are checked at each `rk check` of a workspace that reads it.

use db::RootDatabase;
use rstest::rstest;

use super::{Int, execute_wasm, ordered, with_db};
use crate::tests::codegen::compile_with_libraries;

const SELECTION: &str = include_str!("../../../../stdlib/Selection.st");

/// The library without its `{test}`s, which a test here has no use for checking.
fn selection() -> String {
    let tests = SELECTION
        .find("\tNAMESPACE Test")
        .expect("the library's tests");
    format!("{}END_NAMESPACE\n", &SELECTION[..tests])
}

/// Each expression holds, in a function with `locals`, in a program of
/// `declarations`.
pub(super) fn assert_holds(
    db: &mut RootDatabase,
    declarations: &str,
    locals: &str,
    holds: &[String],
) {
    let mut source = format!(
        "USING Std.Selection;\n{declarations}\nFUNCTION test : DINT\nVAR {locals} END_VAR\n"
    );
    for (i, expression) in holds.iter().enumerate() {
        source += &format!(
            "    IF NOT ({expression}) THEN test := {}; RETURN; END_IF;\n",
            i + 1
        );
    }
    source += "END_FUNCTION\n";
    let wasm = compile_with_libraries(db, &[&selection()], &source);
    let failed: i32 = execute_wasm(&wasm, "test", ());
    if failed != 0 {
        panic!("{} is false", holds[failed as usize - 1]);
    }
}

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
        "k : INT := 1;",
        &[
            format!("MUX(0, {a}, {b}, {c}) = {a}"),
            format!("MUX(k, {a}, {b}, {c}) = {b}"),
            format!("MUX(2, {a}, {b}, {c}) = {c}"),
            format!("MUX(-1, {a}, {b}, {c}) = {a}"),
            format!("MUX(3, {a}, {b}, {c}) = {c}"),
            format!("MUX(0, {a}, {b}) = {a}"),
            format!("MUX(1, {a}, {b}, {c}, {a}) = {b}"),
            format!("MUX(4, {a}, {b}, {c}, {a}, {c}) = {c}"),
        ],
    );
}

/// A selector of any integer type but a ULINT, which a LINT does not hold.
#[rstest]
#[case::sint("SINT")]
#[case::int("INT")]
#[case::dint("DINT")]
#[case::lint("LINT")]
#[case::usint("USINT")]
#[case::uint("UINT")]
#[case::udint("UDINT")]
fn at_each_selector(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let t = Int::of(ty);
    let (one, top, bottom) = (t.lit(1), t.lit(t.max()), t.lit(t.min()));
    assert_holds(
        &mut with_db,
        "",
        "",
        &[
            format!("MUX({one}, 10, 20, 30) = 20"),
            format!("MUX({top}, 10, 20, 30) = 30"),
            format!("MUX({bottom}, 10, 20, 30) = 10"),
        ],
    );
}

/// Over the types the conformance suite's `ops/mux_types` passes, with the values
/// its trace derives from Table 32.
#[rstest]
#[case::first(0, "10", "16#0F", "16#1234", "TRUE", "T#1s")]
#[case::second(1, "65535", "16#F0", "16#FFFF", "FALSE", "T#250ms")]
#[case::third(2, "7", "16#AA", "16#0000", "TRUE", "T#0s")]
fn over_variables(
    mut with_db: RootDatabase,
    #[case] k: i32,
    #[case] mu: &str,
    #[case] mb: &str,
    #[case] mw: &str,
    #[case] mx: &str,
    #[case] mt: &str,
) {
    assert_holds(
        &mut with_db,
        "",
        &format!(
            "k : INT := {k};
    u0 : UINT := 10; u1 : UINT := 65535; u2 : UINT := 7;
    b0 : BYTE := 16#0F; b1 : BYTE := 16#F0; b2 : BYTE := 16#AA;
    w0 : WORD := 16#1234; w1 : WORD := 16#FFFF; w2 : WORD := 16#0000;
    t0 : TIME := T#1s; t1 : TIME := T#250ms; t2 : TIME := T#0s;"
        ),
        &[
            format!("MUX(k, u0, u1, u2) = UINT#{mu}"),
            format!("MUX(k, b0, b1, b2) = BYTE#{mb}"),
            format!("MUX(k, w0, w1, w2) = WORD#{mw}"),
            format!("MUX(k, TRUE, FALSE, TRUE) = {mx}"),
            format!("MUX(k, t0, t1, t2) = {mt}"),
        ],
    );
}

/// A `STRING` has no pack: two, three or four inputs, each with its overload.
#[rstest]
fn on_strings(mut with_db: RootDatabase) {
    assert_holds(
        &mut with_db,
        "",
        "k : DINT := 1; s : STRING[255] := 'var';",
        &[
            "MUX(k, 'a', 'b') = 'b'".to_string(),
            "MUX(0, 'a', 'b') = 'a'".to_string(),
            "MUX(k, 'a', 'b', 'c') = 'b'".to_string(),
            "MUX(2, 'a', 'b', 'c', 'd') = 'c'".to_string(),
            "MUX(9, 'a', 'b', 'c', 'd') = 'd'".to_string(),
            "MUX(-1, 'a', 'b', 'c', 'd') = 'a'".to_string(),
            "MUX(k, 'a', s) = 'var'".to_string(),
        ],
    );
}

/// Inputs are expressions, nested calls included, and each one runs, in order,
/// after the selector: the call has its arguments before it selects.
#[rstest]
fn over_expressions(mut with_db: RootDatabase) {
    assert_holds(
        &mut with_db,
        "FUNCTION Next : INT
VAR_IN_OUT c : INT; END_VAR
    c := c + 1;
    Next := c;
END_FUNCTION

FUNCTION Runs : INT
VAR c : INT; picked : INT; END_VAR
    picked := MUX(Next(c), Next(c), Next(c), Next(c));
    Runs := picked * 10 + c;
END_FUNCTION",
        "k : INT; a : INT := 3; b : INT := 5;",
        &[
            "MUX(k, a + b, a * 2, MAX(a, b)) = 8".to_string(),
            "MUX(k + 1, a + b, a * 2, MAX(a, b)) = 6".to_string(),
            "MUX(k + 2, a + b, a * 2, MAX(a, b)) = 5".to_string(),
            "MUX(1, MUX(0, a, b), MUX(1, a, b)) = 5".to_string(),
            "MUX(MUX(1, 0, 1), a, b) = 5".to_string(),
            "Runs() = 34".to_string(),
        ],
    );
}

/// Inputs of two types land on the wider, as anywhere an overload is picked, and
/// a call of one input or of many selects as any other.
#[rstest]
fn picks_its_overload(mut with_db: RootDatabase) {
    assert_holds(
        &mut with_db,
        "",
        "k : INT := 1; i : INT := -1; d : DINT := 70000; u : UINT := 60000; r : REAL := 2.5; ss : SINT := 7;",
        &[
            "MUX(k, i, d) = DINT#70000".to_string(),
            "MUX(0, i, d) = DINT#-1".to_string(),
            "MUX(k, i, DINT#70000) = DINT#70000".to_string(),
            "MUX(k, ss, i) = INT#-1".to_string(),
            "MUX(k, u, 1) = UINT#1".to_string(),
            "MUX(k, r, 1.5) = REAL#1.5".to_string(),
            "MUX(0, 7) = 7".to_string(),
            "MUX(5, 7) = 7".to_string(),
            "MUX(k, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) = 2"
                .to_string(),
            "MUX(19, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) = 20"
                .to_string(),
        ],
    );
}

/// `K` by name, on either side of the inputs.
#[rstest]
fn with_its_selector_by_name(mut with_db: RootDatabase) {
    assert_holds(
        &mut with_db,
        "",
        "",
        &[
            "MUX(K := 1, 10, 20) = 20".to_string(),
            "MUX(10, 20, K := 1) = 20".to_string(),
        ],
    );
}
