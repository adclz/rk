//! The property every formatter change must keep: **formatting does not change
//! what a program means**.
//!
//! This exists because the suite once asserted the opposite. `BOOL READ_ONLY`
//! was formatted to `BOOLREAD_ONLY` — a type that does not exist — and that
//! output was pinned as an accepted snapshot, so the corruption was *guarded*
//! rather than caught.
//!
//! Idempotence did not catch it either, and could not: the formatter WAS
//! idempotent throughout. `fmt(BOOLREAD_ONLY)` is `BOOLREAD_ONLY`, stably,
//! forever. Converging is not the same as converging on something that parses,
//! and a snapshot test only pins whatever the formatter happens to emit today.
//!
//! So the check here is behavioural, not textual: format a source, then compare
//! the DIAGNOSTICS of the original and the formatted text. If they differ, the
//! formatter changed the program's meaning, whatever the output looks like.
//! Every construct below is a real IEC form; the first four are the ones that
//! were actually being destroyed.

use crate::tests::utils::{add_sources, test_diagnostics, with_db};
use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use rstest::rstest;

use super::formatter::fmt;

/// The diagnostic IDENTITIES in a rendered report: the `[CODE] Severity:
/// summary` headers and each annotation's message, with every trace of
/// position removed.
///
/// Formatting reflows text, so lines, columns, indentation and therefore
/// ariadne's source excerpts and caret alignment all legitimately move. A
/// comparison that included any of them would fail on correct output and teach
/// the next reader to ignore this test. What must NOT move is the set of
/// problems the compiler reports, so only the messages are kept.
fn diagnostic_identities(rendered: &str) -> String {
    let mut out: Vec<String> = rendered
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.starts_with('[') && t.contains(']') {
                // `[E0204] Error: no item found in scope`
                return Some(t.to_string());
            }
            // "`------ no item \"STATION_1\" found in scope" — the annotation
            // text, whose dash run encodes width and so is dropped.
            let (_, msg) = t.split_once('`')?;
            let msg = msg.trim_start_matches('-').trim();
            (!msg.is_empty()).then(|| msg.to_string())
        })
        .collect();
    // Order follows source position, which formatting may also change.
    out.sort();
    out.join("\n")
}

/// Format `source` and return `(before, after)` diagnostics, position-free.
///
/// Each side gets its own database: diagnostics are reported per file, and
/// registering both texts in one database would make every duplicate POU name
/// collide and drown the comparison.
fn diagnostics_across_formatting(source: &str) -> (String, String) {
    let before = {
        let mut db = RootDatabase::default();
        test_diagnostics(&mut db, &[source]).to_string()
    };
    let formatted = {
        let mut db = RootDatabase::default();
        add_sources(&mut db, &[source]);
        let file = *db.get_files().iter().next().expect("one file");
        fmt(file.document(&db))
    };
    let after = {
        let mut db = RootDatabase::default();
        test_diagnostics(&mut db, &[&formatted]).to_string()
    };
    (diagnostic_identities(&before), diagnostic_identities(&after))
}

/// Assert that formatting `source` left its meaning alone, printing the
/// formatted text on failure — the output is the evidence.
#[track_caller]
pub(crate) fn assert_meaning_preserved(label: &str, source: &str) {
    let (before, after) = diagnostics_across_formatting(source);
    if before != after {
        let mut db = RootDatabase::default();
        add_sources(&mut db, &[source]);
        let file = *db.get_files().iter().next().expect("one file");
        panic!(
            "formatting changed the meaning of `{label}`.\n\
             \n--- diagnostics BEFORE ---\n{before}\
             \n--- diagnostics AFTER ----\n{after}\
             \n--- formatted text ------\n{}\n",
            fmt(file.document(&db))
        );
    }
}

/// Located variables: `AT` must keep the space before it, or it glues onto the
/// variable name and the section stops parsing.
#[rstest]
fn located_variables_survive_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "located variable",
        r#"
PROGRAM pgm
VAR
    MYBIT : BOOL;
END_VAR
VAR
    VALVE_POS AT %QW28 : INT;
END_VAR
    MYBIT := TRUE;
END_PROGRAM
"#,
    );
}

/// Edge qualifiers: `BOOL R_EDGE` glued to `BOOLR_EDGE` is a type that does not
/// exist, and the edge trigger is silently gone with it.
#[rstest]
fn edge_qualifiers_survive_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "R_EDGE / F_EDGE",
        r#"
FUNCTION_BLOCK Edges
VAR_INPUT
    Rising : BOOL R_EDGE;
    Falling : BOOL F_EDGE;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

/// VAR_ACCESS directions: the case the suite used to pin BACKWARDS.
#[rstest]
fn access_directions_survive_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "READ_ONLY / READ_WRITE",
        r#"
PROGRAM myPrg
VAR_ACCESS
    ABLE: STATION_1.%IX1.1: BOOL READ_ONLY;
    BAKER: STATION_1.P1.x2: UINT READ_WRITE;
END_VAR
END_PROGRAM
"#,
    );
}

/// A duration's sign belongs to the literal. Spaced apart it becomes a binary
/// operator applied to a bare unit, which no longer type-checks.
#[rstest]
fn signed_duration_literals_survive_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "signed TIME / LTIME",
        r#"
FUNCTION f : TIME
VAR
    a : TIME := T#-14ms;
    b : TIME := T#+250ms;
    c : LTIME := LTIME#-3s;
END_VAR
    a := T#-14ms;
    f := a;
END_FUNCTION
"#,
    );
}

/// The whole temporal-literal family, in one place.
///
/// Only `time`/`ltime` carry a separate sign token, which is why only they
/// broke — but the rest are pinned alongside so a future spacing rule cannot
/// quietly reach TOD/DATE/DT either. `T#+14ms` appears in NO corpus we have,
/// so nothing but a hand-written case can cover it.
#[rstest]
fn every_temporal_literal_survives_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "temporal literals",
        r#"
FUNCTION temporal : BOOL
VAR
    a : TIME := T#14ms;
    b : TIME := T#-14ms;
    c : TIME := T#+14ms;
    d : TIME := TIME#1d2h3m4s5ms;
    e : TIME := t#500us;
    f : LTIME := LTIME#-3s;
    g : LTIME := LT#3ns;
    h : TIME_OF_DAY := TOD#15:36:55.36;
    i : DATE := D#1984-06-25;
    j : DATE_AND_TIME := DT#1984-06-25-15:36:55.36;
    k : INT := INT#-30;
    l : REAL := REAL#-1.5;
END_VAR
    a := T#-14ms;
    temporal := TRUE;
END_FUNCTION
"#,
    );
}

/// A POU with no VAR block at all — the shape whose body used to be joined
/// onto the declaration line.
#[rstest]
fn a_pou_without_variables_survives_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "no VAR block",
        r#"
FUNCTION fn1 : INT
    fn1 := 1;
END_FUNCTION

PROGRAM p
VAR n : INT; END_VAR
    n := fn1();
END_PROGRAM
"#,
    );
}

/// A half-typed loop still formats.
///
/// `DO` is what opens the indentation block `END_FOR` closes, so listing the
/// two independently meant a loop missing its `DO` — every loop, for the
/// seconds you are typing it — closed a block that was never opened, and the
/// WHOLE FILE failed to format. In an editor that formats on save, that is
/// the moment it matters most.
///
/// Formatting an incomplete program is not expected to be pretty; it is
/// expected to happen. These sources are deliberately invalid, so the check is
/// just that meaning is preserved rather than the file being rejected.
#[rstest]
fn a_loop_being_typed_still_formats(#[allow(unused)] with_db: RootDatabase) {
    // Only shapes the grammar RECOVERS from. A source it cannot parse at all
    // (`WHILE a > 0` with no DO and no END_WHILE) is refused by `fmt` on
    // purpose — that is a different thing from an indentation block that was
    // closed without ever being opened.
    for (label, src) in [
        (
            "FOR without DO",
            "FUNCTION fn
VAR i : INT; END_VAR
    FOR i := 0 TO 10
END_FUNCTION
",
        ),
        (
            "FOR with a bad control assignment",
            "FUNCTION fn
    FOR i = p TO smt END_FOR
END_FUNCTION
",
        ),
        (
            "FOR with an arrow instead of an assignment",
            "FUNCTION fn
  VAR i : INT END_VAR
  FOR i => 0 TO 10 END_FOR
END_FUNCTION
",
        ),
    ] {
        assert_meaning_preserved(label, src);
    }
}

/// A broad sweep, so the guard is not limited to the constructs that already
/// bit us: one source exercising the declaration forms, the statement forms,
/// the expression forms and the pragmas together.
#[rstest]
fn a_broad_program_survives_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "broad",
        r#"
TYPE
    Mode : (Idle, Running, Halted);
    Point : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
    Small : INT (0..100);
END_TYPE

FUNCTION_BLOCK Ramp
VAR_INPUT target : DINT; rate : DINT; END_VAR
VAR_OUTPUT value : DINT; END_VAR
VAR i : DINT; acc : DINT; done : BOOL; END_VAR
    acc := 0;
    FOR i := 0 TO 50 BY 2 DO
        acc := acc + rate * 2;
        IF acc > target THEN
            acc := target;
            done := TRUE;
        ELSIF acc < 0 THEN
            acc := 0;
        ELSE
            done := FALSE;
        END_IF;
    END_FOR;
    WHILE acc > target DO
        acc := acc - 1;
    END_WHILE;
    REPEAT
        acc := acc + 1;
    UNTIL acc >= target
    END_REPEAT;
    CASE i OF
        0: value := 0;
        1, 2: value := 1;
    ELSE
        value := acc;
    END_CASE;
END_FUNCTION_BLOCK

FUNCTION mix : REAL
VAR_INPUT a : REAL; b : REAL; END_VAR
VAR t : ARRAY[0..3] OF REAL; s : STRING[10]; END_VAR
    t[0] := a * 2.0 + b / 3.0;
    t[1] := -a;
    s := 'hello';
    mix := t[0] + 7.0;
END_FUNCTION

PROGRAM Main
VAR RETAIN n : DINT; END_VAR
VAR r : Ramp; p : Point; m : Mode; END_VAR
    n := n + 1;
    r(target := 1000, rate := 7);
    p.x := 1;
    m := Mode#Running;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
"#,
    );
}
