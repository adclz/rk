// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// A comparison reads one value on each side, and a STRUCT, an ARRAY or an
// instance has none. These passed the check, then stopped the build with an
// internal error.
#[rstest]
fn invalid_comparison_of_aggregates(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE
        FUNCTION_BLOCK Fb END_FUNCTION_BLOCK

        PROGRAM P
        VAR
            p : Pt; q : Pt;
            a : ARRAY[0..1] OF INT; b : ARRAY[0..1] OF INT;
            f : Fb; g : Fb;
            r : BOOL;
        END_VAR
            r := p = q;
            r := a <> b;
            r := f < g;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:12:18 ]
        |
      2 |         TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE
        |              ^|
        |               `-- 'Pt' is declared here
        |
     12 |             r := p = q;
        |                  ^^|^^
        |                    `---- operator '=' cannot be applied to type 'Pt'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:13:18 ]
        |
      8 |             a : ARRAY[0..1] OF INT; b : ARRAY[0..1] OF INT;
        |             |
        |             `-- 'a' is declared here
        |
     13 |             r := a <> b;
        |                  ^^^|^^
        |                     `---- operator '<>' cannot be applied to type 'ARRAY [0..1] OF INT'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:14:18 ]
        |
      3 |         FUNCTION_BLOCK Fb END_FUNCTION_BLOCK
        |                        ^|
        |                         `-- FUNCTION_BLOCK 'Fb' is declared here
        |
     14 |             r := f < g;
        |                  ^^|^^
        |                    `---- operator '<' cannot be applied to type 'Fb'
    ----'
    ");
}

// Two interfaces are compared no more than two instances: it passed the
// check, then stopped the build with an internal error.
#[rstest]
fn invalid_comparison_of_interfaces(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD M : INT END_METHOD
        END_INTERFACE

        FUNCTION Same : BOOL
        VAR_IN_OUT i1 : I; i2 : I; END_VAR
            Same := i1 = i2;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:8:21 ]
       |
     2 |         INTERFACE I
       |                   |
       |                   `-- INTERFACE 'I' is declared here
       |
     8 |             Same := i1 = i2;
       |                     ^^^|^^^
       |                        `----- operator '=' cannot be applied to type 'I'
    ---'
    ");
}

// AND, OR and XOR combine bits, and a float has none to combine. It used to
// build an invalid module.
#[rstest]
fn invalid_logic_on_floats(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION F : REAL
        VAR a : REAL; b : LREAL; END_VAR
            F := a AND a;
            b := b XOR b;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             F := a AND a;
       |                  ^^^|^^^
       |                     `----- operator 'AND' cannot be applied to type 'REAL'
    ---'
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             b := b XOR b;
       |                  ^^^|^^^
       |                     `----- operator 'XOR' cannot be applied to type 'LREAL'
    ---'
    ");
}

// A sign takes what binary `-` takes. On a BOOL or an enum it made a value
// outside the type; on a STRING or an ARRAY, an internal error.
#[rstest]
fn invalid_sign_on_a_type_without_arithmetic(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green); END_TYPE

        PROGRAM P
        VAR
            b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
            a : ARRAY[0..1] OF INT;
        END_VAR
            b := -b;
            e := -e;
            s := -s;
            c := -c;
            d := -d;
            a := -a;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:9:18 ]
       |
     6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
       |             |
       |             `-- 'b' is declared here
       |
     9 |             b := -b;
       |                  ^|
       |                   `-- operator '-' cannot be applied to type 'BOOL'
    ---'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:10:18 ]
        |
      2 |         TYPE Color : (Red, Green); END_TYPE
        |              ^^|^^
        |                `---- 'Color' is declared here
        |
     10 |             e := -e;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'Color'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:11:18 ]
        |
      6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
        |                                  |
        |                                  `-- 's' is declared here
        |
     11 |             s := -s;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'STRING'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:12:18 ]
        |
      6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
        |                                              |
        |                                              `-- 'c' is declared here
        |
     12 |             c := -c;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'CHAR'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:13:18 ]
        |
      6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
        |                                                        |
        |                                                        `-- 'd' is declared here
        |
     13 |             d := -d;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'DATE'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:14:18 ]
        |
      7 |             a : ARRAY[0..1] OF INT;
        |             |
        |             `-- 'a' is declared here
        |
     14 |             a := -a;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'ARRAY [0..1] OF INT'
    ----'
    ");
}

// Numbers, bit strings and durations keep their sign, as they keep binary
// `-`. A bit string taking arithmetic is an rk extension: IEC 61131-3 gives
// ANY_BIT none.
#[rstest]
fn valid_sign_on_numbers_bit_strings_and_durations(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR i : INT; u : UINT; r : REAL; w : WORD; t : TIME; END_VAR
            i := -i;
            u := -u;
            r := -r;
            w := -w;
            t := -t;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// An untyped literal under a sign or NOT takes its type from the context,
// and the operator is checked against that type there.
#[rstest]
fn invalid_sign_or_not_given_a_type_without_it(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR b : BOOL; i : INT; END_VAR
            b := -(1);
            i := NOT 16#0F;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             b := -(1);
       |                  ^^|^
       |                    `--- operator '-' cannot be applied to type 'BOOL'
    ---'
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             i := NOT 16#0F;
       |                  ^^^^|^^^^
       |                      `------ operator 'NOT' cannot be applied to type 'INT'
    ---'
    ");
}

// NOT on an untyped literal takes the bit string its context gives it: the
// mask idiom `w AND NOT 16#0F0F` was refused, the literal forced to BOOL.
#[rstest]
fn valid_not_on_an_untyped_literal(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR w : WORD; b : BYTE; x : BOOL; END_VAR
            w := w AND NOT 16#0F0F;
            b := NOT 16#0F;
            x := NOT 1;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// An element or a dereference keeps its type under parentheses, a sign
/// or NOT, and as a CASE selector. The parentheses and the unary used to
/// record their operand's base type, the array or the reference, so
/// `x := -a[1]` was a mismatch with 'ARRAY [0..2] OF DINT' and
/// `y + (-a[i])` passed the check to fail in lowering. A field is part of
/// the path and was never affected.
#[rstest]
fn valid_element_and_dereference_under_parentheses_and_unary(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : DINT; END_STRUCT; END_TYPE

FUNCTION f : DINT
VAR
    a : ARRAY[0..2] OF DINT; r : REF_TO DINT; x : DINT; y : DINT; i : INT;
    b : BOOL; bs : ARRAY[0..1] OF BOOL; w : WORD; ws : ARRAY[0..1] OF WORD;
    p : Pt; pr : REF_TO Pt; names : ARRAY[0..1] OF STRING;
END_VAR
    r := REF(x);
    pr := REF(p);
    x := (a[1]);
    x := -a[1];
    x := -(a[1]);
    x := y + (-a[i]);
    x := (a[1]) + 1;
    x := (r^);
    x := -r^;
    b := NOT bs[1];
    b := (bs[0]);
    w := NOT ws[1];
    x := (p.x);
    x := -pr^.x;
    CASE names[i] OF
        'a': x := 1;
    END_CASE;
    CASE a[i] OF
        1: x := 2;
    END_CASE;
    f := x;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An array takes an array and nothing else. A value of its element type
/// used to pass for it: `a := 5` compiled and wrote `a[0]`, and `y + a`
/// or `g(y)` for an array input went through the check into a lowering
/// failure. The array on the left of `+` was already refused.
#[rstest]
fn invalid_scalar_where_an_array_is_expected(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION g : DINT
VAR_INPUT arr : ARRAY[0..2] OF DINT; END_VAR
    g := arr[0];
END_FUNCTION

FUNCTION f : DINT
VAR a : ARRAY[0..2] OF DINT; x : DINT; y : DINT; END_VAR
    a := 5;
    a := y;
    x := y + a;
    x := y * a;
    x := y + (a);
    x := g(y);
    x := g(arr := 5);
    f := x;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:10 ]
       |
     8 | VAR a : ARRAY[0..2] OF DINT; x : DINT; y : DINT; END_VAR
       |     |
       |     `-- 'a' is declared here
     9 |     a := 5;
       |          |
       |          `-- expected 'ARRAY [0..2] OF DINT', got 'INT'
    ---'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:10 ]
        |
      8 | VAR a : ARRAY[0..2] OF DINT; x : DINT; y : DINT; END_VAR
        |     |
        |     `-- 'a' is declared here
        |
     10 |     a := y;
        |          |
        |          `-- expected 'ARRAY [0..2] OF DINT', got 'DINT'
    ----'
    [E0303] Error: types not addable
        ,-[ file:///test0.st:11:14 ]
        |
      8 | VAR a : ARRAY[0..2] OF DINT; x : DINT; y : DINT; END_VAR
        |                                        |
        |                                        `-- 'y' is declared here
        |
     11 |     x := y + a;
        |              |
        |              `-- cannot add 'DINT' with 'ARRAY [0..2] OF DINT'
    ----'
    [E0304] Error: types not multiplicable
        ,-[ file:///test0.st:12:14 ]
        |
      8 | VAR a : ARRAY[0..2] OF DINT; x : DINT; y : DINT; END_VAR
        |                                        |
        |                                        `-- 'y' is declared here
        |
     12 |     x := y * a;
        |              |
        |              `-- cannot multiply 'DINT' with 'ARRAY [0..2] OF DINT'
    ----'
    [E0303] Error: types not addable
        ,-[ file:///test0.st:13:14 ]
        |
      8 | VAR a : ARRAY[0..2] OF DINT; x : DINT; y : DINT; END_VAR
        |                                        |
        |                                        `-- 'y' is declared here
        |
     13 |     x := y + (a);
        |              ^|^
        |               `--- cannot add 'DINT' with 'ARRAY [0..2] OF DINT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:12 ]
        |
      3 | VAR_INPUT arr : ARRAY[0..2] OF DINT; END_VAR
        |           ^|^
        |            `--- 'arr' is declared here
        |
     14 |     x := g(y);
        |            |
        |            `-- expected 'ARRAY [0..2] OF DINT', got 'DINT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:15:19 ]
        |
      3 | VAR_INPUT arr : ARRAY[0..2] OF DINT; END_VAR
        |           ^|^
        |            `--- 'arr' is declared here
        |
     15 |     x := g(arr := 5);
        |                   |
        |                   `-- expected 'ARRAY [0..2] OF DINT', got 'INT'
    ----'
    ");
}
