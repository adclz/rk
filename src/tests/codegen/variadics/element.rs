// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v[i]` reads one argument, counting from 0: the first below that, and
//! the last past the end.

use db::RootDatabase;
use rstest::rstest;

use super::{Int, ordered, run, with_db};

/// Each call gives the value beside it, in a program of `declarations`.
fn assert_calls(db: &mut RootDatabase, declarations: &str, calls: &[(String, String)]) {
    let mut source = format!("{declarations}\nFUNCTION test : DINT\n");
    for (i, (call, want)) in calls.iter().enumerate() {
        source += &format!(
            "    IF {call} <> {want} THEN test := {}; RETURN; END_IF;\n",
            i + 1
        );
    }
    source += "END_FUNCTION\n";
    let failed: i32 = run(db, &source, "test", ());
    if failed != 0 {
        let (call, want) = &calls[failed as usize - 1];
        panic!("{call} is not {want}");
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
    let declarations = format!(
        "FUNCTION F : {ty}\nVAR_INPUT k : LINT; v : {ty}...; END_VAR\n    F := ...v[k];\nEND_FUNCTION\n"
    );
    let call = |k: i64| format!("F(LINT#{k}, {a}, {b}, {c})");
    assert_calls(
        &mut with_db,
        &declarations,
        &[
            (call(0), a.clone()),
            (call(1), b.clone()),
            (call(2), c.clone()),
            (call(-1), a.clone()),
            (call(3), c.clone()),
            (call(i64::MIN), a.clone()),
            (call(i64::MAX), c.clone()),
            (format!("F(0, {b})"), b.clone()),
            (format!("F(7, {b})"), b.clone()),
        ],
    );
}

/// An index widens without changing its value, and a ULINT past the top of a
/// LINT is past the end, not below 0.
#[rstest]
#[case::sint("SINT")]
#[case::int("INT")]
#[case::dint("DINT")]
#[case::lint("LINT")]
#[case::usint("USINT")]
#[case::uint("UINT")]
#[case::udint("UDINT")]
#[case::ulint("ULINT")]
fn at_each_integer_index(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let t = Int::of(ty);
    let declarations = format!(
        "FUNCTION F : INT\nVAR_INPUT k : {ty}; v : INT...; END_VAR\n    F := ...v[k];\nEND_FUNCTION\n"
    );
    let call = |k: i128| format!("F({}, 10, 20, 30)", t.lit(k));
    let at_top = if t.signed { "10" } else { "30" };
    assert_calls(
        &mut with_db,
        &declarations,
        &[
            (call(0), "10".to_string()),
            (call(1), "20".to_string()),
            (call(2), "30".to_string()),
            (call(3), "30".to_string()),
            (call(t.max()), "30".to_string()),
            (call(t.min()), "10".to_string()),
            (call(t.top()), at_top.to_string()),
        ],
    );
}

/// A subrange and an alias index as their base type does, and a pack of an
/// alias reads as the alias.
#[rstest]
fn at_a_subrange_or_an_alias(mut with_db: RootDatabase) {
    let declarations = r#"
TYPE Small : INT(0..3); END_TYPE
TYPE Named : DINT; END_TYPE
TYPE Flag : BOOL; END_TYPE

FUNCTION A : INT
VAR_INPUT k : Small; v : INT...; END_VAR
    A := ...v[k];
END_FUNCTION

FUNCTION B : INT
VAR_INPUT k : Named; v : INT...; END_VAR
    B := ...v[k];
END_FUNCTION

FUNCTION C : Flag
VAR_INPUT k : INT; v : Flag...; END_VAR
    C := ...v[k];
END_FUNCTION
"#;
    assert_calls(
        &mut with_db,
        declarations,
        &[
            ("A(1, 10, 20)".to_string(), "20".to_string()),
            ("A(3, 10, 20)".to_string(), "20".to_string()),
            ("B(-1, 10, 20)".to_string(), "10".to_string()),
            ("C(1, FALSE, TRUE)".to_string(), "TRUE".to_string()),
        ],
    );
}

#[rstest]
fn at_a_constant(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : LINT
VAR_INPUT v : LINT...; END_VAR
VAR CONSTANT two : INT := 2; END_VAR
    F := ...v[0]
        + ...v[1] * 10
        + ...v[two] * 100
        + ...v[1 + 1] * 1000
        + ...v[-5] * 10000
        + ...v[100] * 100000
        + ...v[LINT#5000000000] * 1000000;
END_FUNCTION

FUNCTION test : LINT
    test := F(1, 2, 3);
END_FUNCTION
"#;
    let got: i64 = run(&mut with_db, source, "test", ());
    assert_eq!(got, 3_313_321);
}

/// The index runs once, whatever the number of arguments, one included.
#[rstest]
fn reads_its_index_once(mut with_db: RootDatabase) {
    let declarations = r#"
FUNCTION Next : LINT
VAR_IN_OUT c : INT; END_VAR
    c := c + 1;
    Next := c;
END_FUNCTION

FUNCTION F : INT
VAR_INPUT v : INT...; END_VAR
VAR c : INT; END_VAR
    F := ...v[Next(c)] * 100 + c;
END_FUNCTION
"#;
    assert_calls(
        &mut with_db,
        declarations,
        &[
            ("F(10, 20, 30)".to_string(), "2001".to_string()),
            ("F(10, 20)".to_string(), "2001".to_string()),
            ("F(7)".to_string(), "701".to_string()),
        ],
    );
}

#[rstest]
fn as_an_operand(mut with_db: RootDatabase) {
    let declarations = r#"
FUNCTION G : INT
VAR_INPUT x : INT; END_VAR
    G := x * 2;
END_FUNCTION

FUNCTION F : INT
VAR_INPUT v : INT...; END_VAR
VAR a : ARRAY[0..2] OF INT := [100, 200, 300]; i : INT := 1; END_VAR
    IF ...v[0] > 0 AND -...v[1] < 0 THEN
        F := ...v[0] + ...v[1] * 2 + G(...v[2]) + a[...v[0]] + ...v[...v[0]] + ...v[i + 1];
    END_IF;
END_FUNCTION
"#;
    assert_calls(
        &mut with_db,
        declarations,
        &[
            ("F(1, 2, 3)".to_string(), "216".to_string()),
            ("F(-1, 2, 3)".to_string(), "0".to_string()),
        ],
    );
}

/// Beside the operators that keep their operands in the same scratch: its index
/// is a division, and it is an operand of one and of a fold.
#[rstest]
fn beside_a_division_and_a_fold(mut with_db: RootDatabase) {
    let declarations = r#"
FUNCTION F : LINT
VAR_INPUT v : LINT...; END_VAR
    F := ...v[0] / ...v[1] + ...v[...v[0] / ...v[1]] + (...v MIN) * 100 + (...v MAX) * 1000;
END_FUNCTION
"#;
    assert_calls(
        &mut with_db,
        declarations,
        &[("F(6, 3, 5)".to_string(), "6307".to_string())],
    );
}

/// In a METHOD, which reads the instance beside the pack, and in a FUNCTION's
/// initializer, which runs at each call.
#[rstest]
fn in_a_method_and_an_initializer(mut with_db: RootDatabase) {
    let declarations = r#"
FUNCTION_BLOCK Fb
VAR base : INT := 1000; END_VAR
METHOD PUBLIC Pick : INT
VAR_INPUT k : LINT; v : INT...; END_VAR
    Pick := base + ...v[k];
END_METHOD
END_FUNCTION_BLOCK

FUNCTION Init : INT
VAR_INPUT v : INT...; END_VAR
VAR first : INT := ...v[0]; END_VAR
    Init := first * 100 + ...v[1];
END_FUNCTION

FUNCTION UseMethod : INT
VAR_INPUT k : LINT; END_VAR
VAR fb : Fb; END_VAR
    UseMethod := fb.Pick(k, 1, 2, 3) + fb.Pick(k := k, 4, 5);
END_FUNCTION
"#;
    assert_calls(
        &mut with_db,
        declarations,
        &[
            ("UseMethod(0)".to_string(), "2005".to_string()),
            ("UseMethod(1)".to_string(), "2007".to_string()),
            ("Init(4, 5)".to_string(), "405".to_string()),
            ("Init(6, 7)".to_string(), "607".to_string()),
        ],
    );
}

/// The inputs before a pack take their arguments first, by position or by
/// name, a default included. An output and an in-out are named anywhere.
#[rstest]
fn with_inputs_before_it(mut with_db: RootDatabase) {
    let declarations = r#"
FUNCTION Two : DINT
VAR_INPUT a : DINT; b : DINT := 5; v : DINT...; END_VAR
    Two := a * 1000 + b * 100 + ...v[0] * 10 + ...v+;
END_FUNCTION

FUNCTION Named : DINT
VAR_INPUT k : LINT; v : DINT...; END_VAR
VAR_IN_OUT c : DINT; END_VAR
VAR_OUTPUT o : DINT; END_VAR
    c := c + 1;
    o := ...v[k] * 2;
    Named := ...v[k];
END_FUNCTION

FUNCTION UseNamed : DINT
VAR c : DINT := 10; o : DINT; END_VAR
    UseNamed := Named(1, 20, 30, c := c, o => o);
    UseNamed := UseNamed * 10000 + c * 100 + o;
END_FUNCTION
"#;
    assert_calls(
        &mut with_db,
        declarations,
        &[
            ("Two(1, 2, 3)".to_string(), "1233".to_string()),
            ("Two(1, 2, 3, 4)".to_string(), "1237".to_string()),
            ("Two(b := 9, a := 1, 3)".to_string(), "1933".to_string()),
            ("Two(3, a := 1, b := 2)".to_string(), "1233".to_string()),
            ("UseNamed()".to_string(), "301160".to_string()),
        ],
    );
}

/// One copy per argument count, each with the parameters of its own.
#[rstest]
fn at_each_argument_count(mut with_db: RootDatabase) {
    let declarations = r#"
FUNCTION F : INT
VAR_INPUT k : INT; v : INT...; END_VAR
    F := ...v[k];
END_FUNCTION
"#;
    let calls: Vec<(String, String)> = (1..=12)
        .map(|count: i32| {
            let args: Vec<String> = (1..=count).map(|i| (i * 10).to_string()).collect();
            (
                format!("F({}, {})", count - 1, args.join(", ")),
                (count * 10).to_string(),
            )
        })
        .collect();
    assert_calls(&mut with_db, declarations, &calls);
}

/// A variadic parameter named in a call takes that one argument, beside the ones
/// passed by position.
#[rstest]
fn a_pack_named_in_a_call_takes_one_argument(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Sum : INT
        VAR_INPUT values : INT...; END_VAR
            Sum := ...values+;
        END_FUNCTION

        FUNCTION Pick : INT
        VAR_INPUT k : INT; values : INT...; END_VAR
            Pick := ...values[k];
        END_FUNCTION

        FUNCTION test : DINT
            IF Sum(values := 5) <> 5 THEN test := 1; RETURN; END_IF;
            IF Sum(1, values := 7) <> 8 THEN test := 2; RETURN; END_IF;
            IF Pick(k := 0, values := 7) <> 7 THEN test := 3; RETURN; END_IF;
            IF Pick(values := 7, k := 0) <> 7 THEN test := 4; RETURN; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "test", ());
    assert_eq!(result, 0, "the call that went wrong");
}
