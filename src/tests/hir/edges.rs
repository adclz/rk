// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `R_EDGE` and `F_EDGE` on the inputs of a FUNCTION_BLOCK or a PROGRAM. An
//! edge input reads as a BOOL, its edge, wherever a BOOL does: it had a type
//! of its own, `BOOL (R_EDGE)`, that no assignment, condition or operator
//! took. A FUNCTION or METHOD keeps no value from one call to the next to
//! detect an edge with (E0210), and only a BOOL has one (E0321).

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

#[rstest]
fn valid_edge_inputs(mut with_db: RootDatabase) {
    let source = r#"
TYPE Flag : BOOL; END_TYPE

FUNCTION_BLOCK Fb
VAR_INPUT
    start : BOOL R_EDGE;
    stop : Flag F_EDGE;
    level : BOOL;
END_VAR
VAR_OUTPUT q : BOOL; n : INT; END_VAR
    q := start;
    q := NOT stop;
    q := start AND level OR stop;
    q := start = level;
    IF start THEN n := n + 1; END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR_INPUT pulse : BOOL R_EDGE; END_VAR
VAR fb : Fb; count : INT; END_VAR
    fb(start := pulse, stop := TRUE, level := FALSE);
    IF pulse THEN count := count + 1; END_IF;
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_edge_input_in_a_function_or_a_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : BOOL
VAR_INPUT a : BOOL R_EDGE; END_VAR
    F := a;
END_FUNCTION

FUNCTION_BLOCK Fb
METHOD PUBLIC m
VAR_INPUT b : BOOL F_EDGE; END_VAR
END_METHOD
END_FUNCTION_BLOCK

INTERFACE I
METHOD n
VAR_INPUT c : BOOL R_EDGE; END_VAR
END_METHOD
END_INTERFACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: edge input in a stateless POU
       ,-[ file:///test0.st:3:11 ]
       |
     3 | VAR_INPUT a : BOOL R_EDGE; END_VAR
       |           |
       |           `-- 'a' cannot be R_EDGE in a FUNCTION
       |
       | Help: detect it with an R_TRIG from Std.Edge
       |
       | Note: only a FUNCTION_BLOCK or a PROGRAM keeps an input's value from one call to the next
    ---'
    [E0210] Error: edge input in a stateless POU
       ,-[ file:///test0.st:9:11 ]
       |
     9 | VAR_INPUT b : BOOL F_EDGE; END_VAR
       |           |
       |           `-- 'b' cannot be F_EDGE in a METHOD
       |
       | Help: detect it with an F_TRIG from Std.Edge
       |
       | Note: only a FUNCTION_BLOCK or a PROGRAM keeps an input's value from one call to the next
    ---'
    [E0210] Error: edge input in a stateless POU
        ,-[ file:///test0.st:15:11 ]
        |
     15 | VAR_INPUT c : BOOL R_EDGE; END_VAR
        |           |
        |           `-- 'c' cannot be R_EDGE in a METHOD prototype
        |
        | Help: detect it with an R_TRIG from Std.Edge
        |
        | Note: only a FUNCTION_BLOCK or a PROGRAM keeps an input's value from one call to the next
    ----'
    ");
}

#[rstest]
fn invalid_edge_on_a_type_other_than_bool(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Fb
VAR_INPUT
    n : INT R_EDGE;
    w : WORD F_EDGE;
    p : Pt R_EDGE;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0321] Error: edge qualifier on a type other than BOOL
       ,-[ file:///test0.st:6:9 ]
       |
     6 |     n : INT R_EDGE;
       |         ^|^
       |          `--- expected 'BOOL', got 'INT'
       |
       | Note: R_EDGE detects a change of a BOOL
    ---'
    [E0321] Error: edge qualifier on a type other than BOOL
       ,-[ file:///test0.st:7:9 ]
       |
     7 |     w : WORD F_EDGE;
       |         ^^|^
       |           `--- expected 'BOOL', got 'WORD'
       |
       | Note: F_EDGE detects a change of a BOOL
    ---'
    [E0321] Error: edge qualifier on a type other than BOOL
       ,-[ file:///test0.st:8:9 ]
       |
     8 |     p : Pt R_EDGE;
       |         ^|
       |          `-- expected 'BOOL', got 'Pt'
       |
       | Note: R_EDGE detects a change of a BOOL
    ---'
    ");
}

/// In its own block, an edge input stands for its edge, computed for the
/// call: writing it, referencing it or binding it by reference would leave
/// open whether the input or its edge was meant.
#[rstest]
fn invalid_edge_input_written_or_referenced(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Flip
VAR_IN_OUT v : BOOL; END_VAR
    v := NOT v;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Source
VAR_OUTPUT q : BOOL; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Latch
VAR_INPUT start : BOOL R_EDGE; END_VAR
VAR p : REF_TO BOOL; f : Flip; s : Source; END_VAR
    start := FALSE;
    THIS.start := TRUE;
    p := REF(start);
    f(v := start);
    s(q => start);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: edge input written or referenced
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     start := FALSE;
        |     ^^|^^
        |       `---- the edge of 'start' cannot be written
        |
        | Note: in its own block, an edge input is the edge computed for the call
    ----'
    [E0211] Error: edge input written or referenced
        ,-[ file:///test0.st:15:5 ]
        |
     15 |     THIS.start := TRUE;
        |     ^^^^^|^^^^
        |          `------ the edge of 'start' cannot be written
        |
        | Note: in its own block, an edge input is the edge computed for the call
    ----'
    [E0211] Error: edge input written or referenced
        ,-[ file:///test0.st:16:14 ]
        |
     16 |     p := REF(start);
        |              ^^|^^
        |                `---- the edge of 'start' cannot be referenced
        |
        | Note: in its own block, an edge input is the edge computed for the call
    ----'
    [E0211] Error: edge input written or referenced
        ,-[ file:///test0.st:17:12 ]
        |
     17 |     f(v := start);
        |            ^^|^^
        |              `---- the edge of 'start' cannot be passed to a VAR_IN_OUT
        |
        | Note: in its own block, an edge input is the edge computed for the call
    ----'
    [E0211] Error: edge input written or referenced
        ,-[ file:///test0.st:18:12 ]
        |
     18 |     s(q => start);
        |            ^^|^^
        |              `---- the edge of 'start' cannot be written
        |
        | Note: in its own block, an edge input is the edge computed for the call
    ----'
    ");
}

/// Through an instance, the input is the one the caller gives: it can be
/// written, referenced and passed to a VAR_IN_OUT like any other.
#[rstest]
fn valid_edge_input_as_storage_outside_its_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Flip
VAR_IN_OUT v : BOOL; END_VAR
    v := NOT v;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Latch
VAR_INPUT start : BOOL R_EDGE; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR l : Latch; f : Flip; p : REF_TO BOOL; END_VAR
    l.start := TRUE;
    p := REF(l.start);
    f(v := l.start);
    l();
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
