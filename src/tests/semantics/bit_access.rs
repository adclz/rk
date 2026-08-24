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
       |             `-- type is declared by variable 's' here
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
       |             `-- type is declared by variable 'b' here
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
    [E0229] Error: multibit access out of range
       ,-[ file:///test0.st:6:18 ]
       |
     4 |             b : BYTE;
       |             ^^^^|^^^
       |                 `----- 'b' is declared here
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
    [E0229] Error: multibit access out of range
       ,-[ file:///test0.st:6:18 ]
       |
     4 |             d : DWORD;
       |             ^^^^|^^^^
       |                 `------ 'd' is declared here
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
    [E0229] Error: multibit access out of range
       ,-[ file:///test0.st:6:18 ]
       |
     4 |             w : WORD;
       |             ^^^^|^^^
       |                 `----- 'w' is declared here
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
       |             `-- type is declared by variable 'b' here
       |
     7 |             b.0 := s;
       |                    |
       |                    `-- expected 'BOOL', got 'STRING'
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
