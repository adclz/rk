// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Partial (bit / byte / word) variable access — typing and bounds.
//!
//! IEC 61131-3 §6.5.5. `v.%<size><n>` selects the n-th `<size>`-wide slice of
//! `v`; a bare `v.<n>` is `v.%X<n>`. The access takes the *slice's* type, not
//! the base's, and its offset is checked against the base type's width.
//! Codegen for these lives in `tests::codegen::bit_access`.

use crate::tests::utils::{test_diagnostics, with_db};
use insta::assert_snapshot;
use rstest::*;

/// Every slice size is accepted when it fits the base type.
#[rstest]
fn valid_slice_sizes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            l : LWORD;
            bit : BOOL;
            bt : BYTE;
            w : WORD;
            d : DWORD;
        END_VAR
            bit := l.0;
            bit := l.%X63;
            bt := l.%B7;
            w := l.%W3;
            d := l.%D1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A bit access is a BOOL regardless of the base type's width, so it may be
/// used directly as a condition.
#[rstest]
fn bit_access_is_bool(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            w : WORD;
        END_VAR
            IF w.3 THEN f := 1; END_IF;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// ...and it is *only* a BOOL: it does not inherit the base's bit-string type,
/// so assigning it into a WORD-typed slot without a widening is a type error.
#[rstest]
fn bit_access_is_not_the_base_type(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            w : WORD;
            s : STRING;
        END_VAR
            s := w.3;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:18 ]
       |
     5 |             s : STRING;
       |             |
       |             `-- 's' is declared here
       |
     7 |             s := w.3;
       |                  ^|^
       |                   `--- expected 'STRING', got 'BOOL'
    ---'
    ");
}

/// `%B` yields a BYTE, which is why it can feed a BYTE without a cast but not
/// a narrower slot.
#[rstest]
fn byte_slice_is_byte(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            d : DWORD;
            b : BOOL;
        END_VAR
            b := d.%B1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:18 ]
       |
     5 |             b : BOOL;
       |             |
       |             `-- 'b' is declared here
       |
     7 |             b := d.%B1;
       |                  ^^|^^
       |                    `---- expected 'BOOL', got 'BYTE'
    ---'
    ");
}

/// The offset is checked against the base type's width: a BYTE has bits 0..7.
#[rstest]
fn bit_offset_past_the_end_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            b : BYTE;
        END_VAR
            f := b.8;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1429] Error: partial access out of range
       ,-[ file:///test0.st:6:18 ]
       |
     4 |             b : BYTE;
       |             |
       |             `-- 'b' is declared here
       |
     6 |             f := b.8;
       |                  |
       |                  `-- offset 8 is out of range for type 'BYTE' (valid range: 0..7)
    ---'
    ");
}

/// The bound scales with the slice size, not the bit count: a DWORD holds four
/// bytes, so `%B4` is one past the end even though bit 4 exists.
#[rstest]
fn byte_offset_past_the_end_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BYTE
        VAR
            d : DWORD;
        END_VAR
            f := d.%B4;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1429] Error: partial access out of range
       ,-[ file:///test0.st:6:18 ]
       |
     4 |             d : DWORD;
       |             |
       |             `-- 'd' is declared here
       |
     6 |             f := d.%B4;
       |                  |
       |                  `-- offset 4 is out of range for type 'DWORD' (valid range: 0..3)
    ---'
    ");
}

/// A slice wider than the base has no valid offset at all.
#[rstest]
fn slice_wider_than_base_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : LWORD
        VAR
            w : WORD;
        END_VAR
            f := w.%D0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1429] Error: partial access out of range
       ,-[ file:///test0.st:6:18 ]
       |
     4 |             w : WORD;
       |             |
       |             `-- 'w' is declared here
       |
     6 |             f := w.%D0;
       |                  |
       |                  `-- a 32-bit access does not fit in type 'WORD'
    ---'
    ");
}

/// A partial access is a place, so it is assignable — the write is a
/// read-modify-write of the base (see the codegen tests).
#[rstest]
fn slice_is_assignable(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BYTE
        VAR
            b : BYTE;
            d : DWORD;
        END_VAR
            b.0 := TRUE;
            b.%X7 := FALSE;
            d.%B2 := b;
            f := b;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// Assigning through a slice type-checks against the *slice's* type.
#[rstest]
fn assigning_wrong_type_through_a_slice_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            b : BYTE;
            s : STRING;
        END_VAR
            b.0 := s;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:20 ]
       |
     4 |             b : BYTE;
       |             |
       |             `-- 'b' is declared here
       |
     7 |             b.0 := s;
       |                    |
       |                    `-- expected 'BOOL', got 'STRING'
    ---'
    ");
}

/// A FUNCTION's own name is its result, and a slice of it is a slice as of a
/// variable: typed as the slice, bounded by the result's type. It was typed
/// as the whole result, so an LWORD went into a BYTE unchecked, and an
/// offset past the result was never refused.
#[rstest]
fn a_slice_of_a_result_is_a_slice(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION F : LWORD
            F.%B7 := LWORD#16#FFFF;
        END_FUNCTION

        FUNCTION H : WORD
            H.%B2 := BYTE#1;
        END_FUNCTION

        FUNCTION Fine : LWORD
            Fine.%B7 := BYTE#1;
            Fine.%X0 := TRUE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:3:22 ]
       |
     2 |         FUNCTION F : LWORD
       |                  |
       |                  `-- 'F' is declared here
     3 |             F.%B7 := LWORD#16#FFFF;
       |                      ^^^^^^|^^^^^^
       |                            `-------- expected 'BYTE', got 'LWORD'
       |
       | Help: insert explicit cast 'LWORD_TO_BYTE(LWORD#16#FFFF)'
    ---'
    [E1429] Error: partial access out of range
       ,-[ file:///test0.st:7:13 ]
       |
     7 |             H.%B2 := BYTE#1;
       |             |
       |             `-- offset 2 is out of range for type 'WORD' (valid range: 0..1)
    ---'
    ");
}

/// Partial access reaches through a path: the slice applies to the member the
/// path lands on, not to the root.
#[rstest]
fn slice_on_a_function_block_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK FB
        VAR
            flags : WORD;
        END_VAR
            flags.15 := TRUE;
        END_FUNCTION_BLOCK

        FUNCTION f : BOOL
        VAR
            inst : FB;
        END_VAR
            f := inst.flags.15;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A size character naming no slice is refused HERE, so lowering never sees it.
///
/// The grammar cannot catch it: `adress_identifier` is shared with direct
/// variables, whose addresses run to `IX`, `QW`, `MD`, so it admits any
/// letters. Without this check the access typed as BOOL — the fallback for an
/// undecodable slice — `rk check` passed, and `rk compile` then failed with an
/// internal compiler error on a program the front end had just accepted.
#[rstest]
fn unknown_slice_size_is_refused(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            w : WORD;
            y : BYTE;
        END_VAR
            y := w.%Z1;
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1418] Error: unknown multibit access size
       ,-[ file:///test0.st:7:18 ]
       |
     7 |             y := w.%Z1;
       |                  |
       |                  `-- '%Z' names no access size (expected X, B, W, D or L)
    ---'
    ");
}

/// The offset is measured against what the slice APPLIES to: the element of
/// `arr[k]`, the field of `s.fld` — not the array or the struct.
///
/// The multibit of a multi-step path is deferred until after the walk, and
/// the bounds check only ran on single-step paths, so nothing measured these
/// at all. They reached lowering, which does read the element's width, and
/// died there with an internal compiler error.
#[rstest]
fn offset_is_bounded_by_the_element_it_slices(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT
            fld : WORD;
        END_STRUCT; END_TYPE

        FUNCTION f : BOOL
        VAR
            arr : ARRAY[0..3] OF BYTE;
            s : S;
            k : INT := 1;
        END_VAR
            f := arr[k].9;
            f := s.fld.16;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1429] Error: partial access out of range
        ,-[ file:///test0.st:12:18 ]
        |
     12 |             f := arr[k].9;
        |                  ^|^
        |                   `--- offset 9 is out of range for type 'BYTE' (valid range: 0..7)
    ----'
    [E1429] Error: partial access out of range
        ,-[ file:///test0.st:13:20 ]
        |
     13 |             f := s.fld.16;
        |                    ^|^
        |                     `--- offset 16 is out of range for type 'WORD' (valid range: 0..15)
    ----'
    ");
}

/// A slice wider than the element it sits on, reached through a subscript.
#[rstest]
fn sized_slice_wider_than_the_element(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            arr : ARRAY[0..3] OF BYTE;
            k : INT := 1;
            w : WORD;
        END_VAR
            w := arr[k].%W1;
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1429] Error: partial access out of range
       ,-[ file:///test0.st:8:18 ]
       |
     8 |             w := arr[k].%W1;
       |                  ^|^
       |                   `--- a 16-bit access does not fit in type 'BYTE'
    ---'
    ");
}

/// The offset must be a literal: there is no runtime-computed slice.
///
/// Both spellings LOOK valid, so both are pinned. `b.i` reads as a field
/// access and is refused as one, with the rule and the way to a computed
/// bit. `b.%Xi` reads as a size `Xi` with no position, a syntax error.
#[rstest]
fn a_variable_offset_is_not_a_slice(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            b : BYTE;
            i : INT := 3;
        END_VAR
            f := b.i;
        END_FUNCTION

        FUNCTION g : BOOL
        VAR
            b : BYTE;
        END_VAR
            g := b.%Xi;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0202] Error: unknown field
       ,-[ file:///test0.st:7:20 ]
       |
     4 |             b : BYTE;
       |             |
       |             `-- 'b' is declared here
       |
     7 |             f := b.i;
       |                    |
       |                    `-- 'BYTE' has no field named 'i'
       |
       | Help: shift the value right by 'i' with 'SHR' to reach a computed bit
       |
       | Note: the position of a partial access is an integer literal
    ---'
    [E0002] Error: missing element
        ,-[ file:///test0.st:14:23 ]
        |
     14 |             g := b.%Xi;
        |                       |
        |                       `- missing 'unsigned_int'
        |                       |
        |                       `- add missing unsigned_int here
    ----'
    ");
}

/// Every integer takes a partial access, signed or not, and so does a
/// subrange of one. A BOOL has one part, its bit 0.
#[rstest]
fn integers_and_bools_take_a_partial_access(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..10); END_TYPE

        FUNCTION f : BOOL
        VAR
            si : SINT; i : INT; di : DINT; li : LINT;
            usi : USINT; ui : UINT; udi : UDINT; uli : ULINT;
            s : Small;
            b : BOOL;
            y : BYTE; w : WORD; d : DWORD; l : LWORD;
        END_VAR
            f := si.7 AND i.%X15 AND di.31 AND li.63;
            f := usi.0 AND ui.15 AND udi.%X31 AND uli.%X63;
            y := i.%B1; w := di.%W1; d := li.%D1; l := uli.%L0;
            y := s.%B0;
            s.0 := TRUE;
            b.0 := f;
            f := b.%X0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// What is not a bit string, an integer or a BOOL has no parts. These went
/// through `check` and failed in the build: a REAL's emitted an invalid
/// module, a STRING's, a STRUCT's or an ARRAY's stopped with an internal
/// compiler error, the others read their encoding. Where a conversion gives
/// the bits, the help names it.
#[rstest]
fn a_type_without_parts_is_refused(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green); END_TYPE
        TYPE Pair : STRUCT a : INT; END_STRUCT; END_TYPE

        FUNCTION f : BOOL
        VAR
            r : REAL;
            lr : LREAL;
            c : CHAR;
            t : TIME;
            e : Color;
            s : STRING;
            p : Pair;
            a : ARRAY[0..3] OF BYTE;
            rp : REF_TO INT;
            w : WORD;
        END_VAR
            f := r.31;
            w := lr.%W3;
            c.0 := TRUE;
            f := t.3;
            f := e.0;
            f := s.0;
            f := p.0;
            f := a.0;
            f := rp.0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:18:18 ]
        |
      7 |             r : REAL;
        |             |
        |             `-- 'r' is declared here
        |
     18 |             f := r.31;
        |                  |
        |                  `-- type 'REAL' has no parts to access
        |
        | Help: convert it with 'REAL_TO_DWORD' and access the parts of the 'DWORD'
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:19:18 ]
        |
      8 |             lr : LREAL;
        |             ^|
        |              `-- 'lr' is declared here
        |
     19 |             w := lr.%W3;
        |                  ^|
        |                   `-- type 'LREAL' has no parts to access
        |
        | Help: convert it with 'LREAL_TO_LWORD' and access the parts of the 'LWORD'
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:20:13 ]
        |
      9 |             c : CHAR;
        |             |
        |             `-- 'c' is declared here
        |
     20 |             c.0 := TRUE;
        |             |
        |             `-- type 'CHAR' has no parts to access
        |
        | Help: convert it with 'CHAR_TO_BYTE' and access the parts of the 'BYTE'
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:21:18 ]
        |
     10 |             t : TIME;
        |             |
        |             `-- 't' is declared here
        |
     21 |             f := t.3;
        |                  |
        |                  `-- type 'TIME' has no parts to access
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:22:18 ]
        |
     11 |             e : Color;
        |             |
        |             `-- 'e' is declared here
        |
     22 |             f := e.0;
        |                  |
        |                  `-- type 'Color' has no parts to access
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:23:18 ]
        |
     12 |             s : STRING;
        |             |
        |             `-- 's' is declared here
        |
     23 |             f := s.0;
        |                  |
        |                  `-- type 'STRING' has no parts to access
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:24:18 ]
        |
     13 |             p : Pair;
        |             |
        |             `-- 'p' is declared here
        |
     24 |             f := p.0;
        |                  |
        |                  `-- type 'Pair' has no parts to access
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:25:18 ]
        |
     14 |             a : ARRAY[0..3] OF BYTE;
        |             |
        |             `-- 'a' is declared here
        |
     25 |             f := a.0;
        |                  |
        |                  `-- type 'ARRAY [0..3] OF BYTE' has no parts to access
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    [E1431] Error: partial access to a type without parts
        ,-[ file:///test0.st:26:18 ]
        |
     15 |             rp : REF_TO INT;
        |             ^|
        |              `-- 'rp' is declared here
        |
     26 |             f := rp.0;
        |                  ^|
        |                   `-- type 'REF_TO INT' has no parts to access
        |
        | Help: dereference it with '^' and access the parts of what it points to
        |
        | Note: a partial access applies to a bit string, an integer or a BOOL
    ----'
    ");
}

/// A size is one letter. Only the first was read, so `%BX1` and `%Bfoo1`
/// passed as `%B1`.
#[rstest]
fn a_size_is_one_letter(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BYTE
        VAR
            w : WORD;
        END_VAR
            f := w.%BX1;
            f := w.%Bfoo1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1418] Error: unknown multibit access size
       ,-[ file:///test0.st:6:18 ]
       |
     6 |             f := w.%BX1;
       |                  |
       |                  `-- '%BX' names no access size (expected X, B, W, D or L)
    ---'
    [E1418] Error: unknown multibit access size
       ,-[ file:///test0.st:7:18 ]
       |
     7 |             f := w.%Bfoo1;
       |                  |
       |                  `-- '%Bfoo' names no access size (expected X, B, W, D or L)
    ---'
    ");
}

/// The size letter is caseless, as every keyword is.
#[rstest]
fn the_size_letter_is_caseless(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        VAR
            l : LWORD;
            y : BYTE; w : WORD; d : DWORD;
        END_VAR
            f := l.%x63;
            y := l.%b7;
            w := l.%w3;
            d := l.%d1;
            l := l.%l0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
