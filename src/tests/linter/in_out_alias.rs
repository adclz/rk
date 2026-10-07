// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// One variable to two VAR_IN_OUTs, a struct and one of its fields, the same
/// element twice, an in-out and an output: each pair reaches the same
/// storage during the call.
#[rstest]
fn same_storage_passed_twice(mut with_db: RootDatabase) {
    let source = r#"
TYPE Buffer : STRUCT len : INT; data : ARRAY[0..9] OF INT; END_STRUCT; END_TYPE

FUNCTION swap
VAR_IN_OUT a : INT; b : INT; END_VAR
VAR t : INT; END_VAR
    t := a;
    a := b;
    b := t;
END_FUNCTION

FUNCTION fill
VAR_IN_OUT whole : Buffer; count : INT; END_VAR
    whole.len := 0;
    count := count + 1;
END_FUNCTION

FUNCTION bump
VAR_IN_OUT io : INT; END_VAR
VAR_OUTPUT o : INT; END_VAR
    io := io + 1;
    o := io;
END_FUNCTION

FUNCTION test : INT
VAR x : INT; buf : Buffer; arr : ARRAY[0..9] OF INT; i : INT; END_VAR
    swap(a := x, b := x);
    fill(whole := buf, count := buf.len);
    swap(a := arr[1], b := arr[1]);
    swap(arr[i], arr[i]);
    bump(io := x, o => x);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "in-out-alias"), @r"
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:27:23 ]
        |
     27 |     swap(a := x, b := x);
        |               |       |
        |               `---------- 'x' is passed to 'a' here
        |                       |
        |                       `-- 'x' is passed to 'b' and to 'a'
        |
        | Help: pass two different variables, or copy one first
        |
        | Note 1: both parameters reach the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:28:33 ]
        |
     28 |     fill(whole := buf, count := buf.len);
        |                   ^|^           ^^^|^^^
        |                    `--------------------- 'buf' is passed to 'whole' here
        |                                    |
        |                                    `----- 'buf.len' is passed to 'count' and to 'whole'
        |
        | Help: pass two different variables, or copy one first
        |
        | Note 1: both parameters reach the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:29:28 ]
        |
     29 |     swap(a := arr[1], b := arr[1]);
        |               ^^^|^^       ^^^|^^
        |                  `----------------- 'arr[1]' is passed to 'a' here
        |                               |
        |                               `---- 'arr[1]' is passed to 'b' and to 'a'
        |
        | Help: pass two different variables, or copy one first
        |
        | Note 1: both parameters reach the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:30:18 ]
        |
     30 |     swap(arr[i], arr[i]);
        |          ^^^|^^  ^^^|^^
        |             `------------ 'arr[i]' is passed to 'a' here
        |                     |
        |                     `---- 'arr[i]' is passed to 'b' and to 'a'
        |
        | Help: pass two different variables, or copy one first
        |
        | Note 1: both parameters reach the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:31:24 ]
        |
     31 |     bump(io := x, o => x);
        |                |       |
        |                `---------- 'x' is passed to 'io' here
        |                        |
        |                        `-- 'x' is passed to 'o' and to 'io'
        |
        | Help: pass two different variables, or copy one first
        |
        | Note 1: both parameters reach the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    ");
}

/// A FUNCTION_BLOCK holds its in-outs for its body: the same variable twice
/// is reported there too.
#[rstest]
fn same_variable_to_a_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Mover
VAR_IN_OUT src : INT; dst : INT; END_VAR
    dst := src;
    src := 0;
END_FUNCTION_BLOCK

PROGRAM Main
VAR m : Mover; v : INT; END_VAR
    m(src := v, dst := v);
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "in-out-alias"), @r"
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:10:24 ]
        |
     10 |     m(src := v, dst := v);
        |              |         |
        |              `------------ 'v' is passed to 'src' here
        |                        |
        |                        `-- 'v' is passed to 'dst' and to 'src'
        |
        | Help: pass two different variables, or copy one first
        |
        | Note 1: both parameters reach the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    ");
}

/// Two places that may differ are not reported: two elements, at constant
/// or computed subscripts, as a sort swaps them; two fields; two variables;
/// an input, which is a copy.
#[rstest]
fn distinct_storage_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT a : INT; b : INT; END_STRUCT; END_TYPE

FUNCTION swap
VAR_IN_OUT a : INT; b : INT; END_VAR
VAR t : INT; END_VAR
    t := a;
    a := b;
    b := t;
END_FUNCTION

FUNCTION add_to
VAR_INPUT v : INT; END_VAR
VAR_IN_OUT acc : INT; END_VAR
    acc := acc + v;
END_FUNCTION

FUNCTION test : INT
VAR arr : ARRAY[0..9] OF INT; i : INT; j : INT; p : Pair; x : INT; y : INT; END_VAR
    swap(a := arr[1], b := arr[2]);
    swap(a := arr[i], b := arr[j]);
    swap(a := p.a, b := p.b);
    swap(a := x, b := y);
    add_to(v := x, acc := x);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "in-out-alias"), @r"");
}

/// A global passed to a VAR_IN_OUT of a callee that also reaches it by
/// VAR_EXTERNAL, itself or through a FUNCTION it calls. A callee that never
/// names the global is not reported.
#[rstest]
fn a_global_the_callee_also_reaches(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION bump
VAR_IN_OUT a : INT; END_VAR
VAR_EXTERNAL g : INT; END_VAR
    a := a + 1;
    g := g * 2;
END_FUNCTION

FUNCTION double_g
VAR_EXTERNAL g : INT; END_VAR
    g := g * 2;
END_FUNCTION

FUNCTION bump_then_double
VAR_IN_OUT a : INT; END_VAR
    a := a + 1;
    double_g();
END_FUNCTION

FUNCTION inc
VAR_IN_OUT a : INT; END_VAR
    a := a + 1;
END_FUNCTION

PROGRAM Main
VAR_EXTERNAL g : INT; END_VAR
    bump(a := g);
    bump_then_double(a := g);
    inc(a := g);
END_PROGRAM

CONFIGURATION Plant
VAR_GLOBAL g : INT; END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "in-out-alias"), @r"
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:27:15 ]
        |
     27 |     bump(a := g);
        |               |
        |               `-- 'g' passed to 'a' is a global 'bump' also reaches
        |
        | Help: pass a copy, or let 'bump' reach it one way only
        |
        | Note 1: the parameter and the global are the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    [L0125] Warning: same storage passed twice by reference
        ,-[ file:///test0.st:28:27 ]
        |
     28 |     bump_then_double(a := g);
        |                           |
        |                           `-- 'g' passed to 'a' is a global 'bump_then_double' also reaches
        |
        | Help: pass a copy, or let 'bump_then_double' reach it one way only
        |
        | Note 1: the parameter and the global are the same storage, so a write through one changes what the other reads
        |
        | Note 2: lint rule: in-out-alias
    ----'
    ");
}
