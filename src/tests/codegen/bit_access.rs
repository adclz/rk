// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Partial (bit / byte / word) variable access — `b.1`, `w.%X3`, `d.%B2`.
//!
//! IEC 61131-3 §6.5.5. Before these landed, MIR ignored the `multibits` part
//! entirely: a read loaded the *whole* variable and merely relabelled it with
//! the slice's type (`b.1` on `BYTE#5` yielded `TRUE` because 5 ≠ 0), and a
//! write stored through the slice onto the whole variable, wiping its other
//! bits. Every test here therefore checks a value that only comes out right if
//! the shift and the mask are both applied.

use crate::tests::codegen::{compile_to_wasm, execute_wasm, run, with_db};
use rstest::*;

/// `2#0000_0101` — bits 0 and 2 set, so a correct read alternates
/// TRUE/FALSE/TRUE and a whole-variable read would answer TRUE for all three.
#[rstest]
#[case("b.0", 1)]
#[case("b.1", 0)]
#[case("b.2", 1)]
#[case("b.3", 0)]
#[case("b.7", 0)]
fn read_bit_of_byte(mut with_db: db::RootDatabase, #[case] access: &str, #[case] expected: i32) {
    let source = format!(
        r#"
        FUNCTION get : BOOL
        VAR
            b : BYTE := 2#0000_0101;
        END_VAR
            get := {access};
        END_FUNCTION
    "#
    );
    let result: i32 = run(&mut with_db, &source, "get", ());
    assert_eq!(result, expected, "{access} on 2#0000_0101");
}

/// `%Xn` is the explicit spelling of the bare `.n` bit access.
#[rstest]
fn read_bit_with_explicit_x(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION get : BOOL
        VAR
            w : WORD := 16#8000;
        END_VAR
            get := w.%X15;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result, 1, "bit 15 of 16#8000 is set");
}

/// A bit access must yield exactly 0 or 1, not "the whole value, retyped".
/// `16#F0` is non-zero, so the old whole-variable read reported every bit set.
#[rstest]
fn read_bit_is_zero_or_one_not_truthiness(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION count_low_nibble : INT
        VAR
            b : BYTE := 16#F0;
            i : INT;
        END_VAR
            IF b.0 THEN count_low_nibble := count_low_nibble + 1; END_IF;
            IF b.1 THEN count_low_nibble := count_low_nibble + 1; END_IF;
            IF b.2 THEN count_low_nibble := count_low_nibble + 1; END_IF;
            IF b.3 THEN count_low_nibble := count_low_nibble + 1; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "count_low_nibble", ());
    assert_eq!(result, 0, "no low-nibble bit of 16#F0 is set");
}

/// `%Bn` selects the n-th *byte*, so the shift is `n * 8`, not `n`.
#[rstest]
#[case("d.%B0", 0x44)]
#[case("d.%B1", 0x33)]
#[case("d.%B2", 0x22)]
#[case("d.%B3", 0x11)]
fn read_byte_of_dword(mut with_db: db::RootDatabase, #[case] access: &str, #[case] expected: i32) {
    let source = format!(
        r#"
        FUNCTION get : BYTE
        VAR
            d : DWORD := 16#11223344;
        END_VAR
            get := {access};
        END_FUNCTION
    "#
    );
    let result: i32 = run(&mut with_db, &source, "get", ());
    assert_eq!(result, expected, "{access} on 16#11223344");
}

/// `%Wn` shifts by `n * 16` and masks 16 bits.
#[rstest]
#[case("d.%W0", 0x3344)]
#[case("d.%W1", 0x1122)]
fn read_word_of_dword(mut with_db: db::RootDatabase, #[case] access: &str, #[case] expected: i32) {
    let source = format!(
        r#"
        FUNCTION get : WORD
        VAR
            d : DWORD := 16#11223344;
        END_VAR
            get := {access};
        END_FUNCTION
    "#
    );
    let result: i32 = run(&mut with_db, &source, "get", ());
    assert_eq!(result, expected, "{access} on 16#11223344");
}

/// Slicing an `LWORD` shifts in i64 and only narrows once the slice is
/// isolated — `%D1` and `%B7` both live above bit 32, so a 32-bit shift would
/// read the wrong half (or nothing at all).
#[rstest]
#[case("l.%D0", 0x55667788u32 as i32)]
#[case("l.%D1", 0x11223344)]
fn read_dword_of_lword(mut with_db: db::RootDatabase, #[case] access: &str, #[case] expected: i32) {
    let source = format!(
        r#"
        FUNCTION get : DWORD
        VAR
            l : LWORD := 16#1122334455667788;
        END_VAR
            get := {access};
        END_FUNCTION
    "#
    );
    let result: i32 = run(&mut with_db, &source, "get", ());
    assert_eq!(result, expected, "{access} on 16#1122334455667788");
}

#[rstest]
#[case("l.%B0", 1)]
#[case("l.%B7", 1)]
#[case("l.%B4", 1)]
#[case("l.%X63", 0)]
#[case("l.%X60", 1)]
#[case("l.%X0", 0)]
#[case("l.%X3", 1)]
fn read_high_slices_of_lword(
    mut with_db: db::RootDatabase,
    #[case] access: &str,
    #[case] expected: i32,
) {
    let source = format!(
        r#"
        FUNCTION get : INT
        VAR
            l : LWORD := 16#1122334455667788;
        END_VAR
            IF {access} <> 0 THEN get := 1; ELSE get := 0; END_IF;
        END_FUNCTION
    "#
    );
    // Compared against 0 rather than returned directly so one body shape covers
    // both the BOOL of `%X` and the BYTE of `%B`; the cases below therefore
    // assert "slice is non-zero", which still fails if the shift is wrong.
    let result: i32 = run(&mut with_db, &source, "get", ());
    assert_eq!(result, expected, "{access} on 16#1122334455667788");
}

/// Writing one bit is a read-modify-write: the other bits of the base must
/// survive. The pre-fix codegen stored the RHS over the whole variable, so
/// `b.0 := TRUE` turned `16#F0` into `1`.
#[rstest]
fn write_bit_preserves_other_bits(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set : BYTE
        VAR
            b : BYTE := 16#F0;
        END_VAR
            b.0 := TRUE;
            set := b;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "set", ());
    assert_eq!(result, 0xF1, "setting bit 0 of 16#F0 gives 16#F1");
}

/// Clearing must clear only the named bit.
#[rstest]
fn write_bit_false_clears_only_that_bit(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION clear : BYTE
        VAR
            b : BYTE := 16#FF;
        END_VAR
            b.3 := FALSE;
            clear := b;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "clear", ());
    assert_eq!(result, 0xF7, "clearing bit 3 of 16#FF gives 16#F7");
}

/// Successive writes accumulate rather than overwrite each other.
#[rstest]
fn write_several_bits_accumulates(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION build : BYTE
        VAR
            b : BYTE := 0;
        END_VAR
            b.0 := TRUE;
            b.2 := TRUE;
            b.5 := TRUE;
            build := b;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "build", ());
    assert_eq!(result, 0b0010_0101, "bits 0, 2 and 5 set");
}

/// Writing a byte slice replaces 8 bits at `n * 8` and leaves the rest.
#[rstest]
fn write_byte_slice_of_dword(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION patch : DWORD
        VAR
            d : DWORD := 16#11223344;
        END_VAR
            d.%B2 := 16#AA;
            patch := d;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "patch", ());
    assert_eq!(
        result, 0x11AA3344u32 as i32,
        "byte 2 replaced, others untouched"
    );
}

/// A write through a slice must not leave stray bits above the base type's
/// width: `BYTE` lives in an i32 lane and its stored form is zero-masked, so a
/// later full read has to see exactly the 8 bits.
#[rstest]
fn write_bit_keeps_subwidth_lane_clean(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION widen : DWORD
        VAR
            b : BYTE := 16#0F;
            d : DWORD;
        END_VAR
            b.7 := TRUE;
            d := b;
            widen := d;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "widen", ());
    assert_eq!(result, 0x8F, "no bits above bit 7 survive in a BYTE");
}

/// Slices of a 64-bit base are written in i64 — the RHS is extended before the
/// mask and shift, so a high bit does not fall off the top of an i32.
#[rstest]
fn write_high_bit_of_lword(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set : LWORD
        VAR
            l : LWORD := 0;
        END_VAR
            l.%X63 := TRUE;
            l.%B0 := 16#FF;
            set := l;
        END_FUNCTION
    "#;
    let result: i64 = run(&mut with_db, source, "set", ());
    assert_eq!(result, 0x8000_0000_0000_00FFu64 as i64);
}

/// Read and write compose: reading a bit back after writing it must observe
/// the write, and a slice of a variable holding a runtime (not literal) value
/// must work the same as one holding an initializer.
#[rstest]
fn read_back_after_write_on_runtime_value(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION roundtrip : BOOL
        VAR_INPUT
            src : BYTE;
        END_VAR
        VAR
            b : BYTE;
        END_VAR
            b := src;
            b.4 := b.0;
            roundtrip := b.4;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let set: i32 = execute_wasm(&wasm, "roundtrip", 0x01);
    assert_eq!(set, 1, "bit 0 of 16#01 is set, so bit 4 becomes set");

    let clear: i32 = execute_wasm(&wasm, "roundtrip", 0x02);
    assert_eq!(clear, 0, "bit 0 of 16#02 is clear, so bit 4 stays clear");
}

/// Partial access on a function-block member goes through the same path as a
/// local: the place is the member slot, the slice arithmetic is identical.
#[rstest]
fn bit_access_on_fb_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Flags
        VAR_INPUT
            seed : BYTE;
        END_VAR
        VAR_OUTPUT
            bits : BYTE;
        END_VAR
            bits := seed;
            bits.0 := TRUE;
            bits.3 := FALSE;
        END_FUNCTION_BLOCK

        FUNCTION run : BYTE
        VAR
            f : Flags;
        END_VAR
            f(seed := 2#0000_1010);
            run := f.bits;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(
        result, 0b0000_0011,
        "2#1010, bit 0 set and bit 3 cleared, bit 1 untouched"
    );
}

/// A slice of an ARRAY ELEMENT, read and written.
///
/// The base of such a slice is the ELEMENT, and nothing carried that width to
/// lowering: HIR handed over the type the slice PRODUCES, so `arr[k].7`
/// measured bit 8 against the 1 bit of a BOOL and lowering refused a program
/// `check` had passed. Offset 0 was the one case that worked, which is why a
/// test reading `arr[k].0` would have proved nothing.
#[rstest]
fn slice_of_an_array_element(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION get : BYTE
        VAR
            arr : ARRAY[0..3] OF BYTE := [16#11, 16#80, 16#33, 16#44];
            k : INT := 1;
        END_VAR
            IF arr[k].7 THEN
                arr[k].0 := TRUE;
            END_IF;
            get := arr[k];
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result, 0x81, "bit 7 of 16#80 is set, so bit 0 is written");
}

/// A slice of a STRUCT FIELD, read and written — the same lost base width as
/// the array case above, reached through a field instead of a subscript.
#[rstest]
fn slice_of_a_struct_field(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT
            fld : WORD;
        END_STRUCT; END_TYPE

        FUNCTION get : WORD
        VAR
            s : S;
        END_VAR
            s.fld := 16#1234;
            s.fld.15 := TRUE;
            get := s.fld;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result, 0x9234, "bit 15 set on 16#1234");
}

/// A sized slice of a struct field reads the field's width, not the slice's.
#[rstest]
fn sized_slice_of_a_struct_field(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT
            fld : WORD;
        END_STRUCT; END_TYPE

        FUNCTION get : BYTE
        VAR
            s : S;
        END_VAR
            s.fld := 16#1234;
            get := s.fld.%B1;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result, 0x12, "byte 1 of 16#1234");
}

/// A slice of a subrange element, field or referenced value, read and
/// written: the slot's width is its base's, lost the same way as above.
#[rstest]
fn slice_of_a_subrange_slot(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : INT (0..200); END_TYPE
        TYPE S : STRUCT
            fld : Small;
        END_STRUCT; END_TYPE

        FUNCTION get : DINT
        VAR
            arr : ARRAY[0..2] OF Small := [0, 128, 0];
            s : S := (fld := 128);
            v : Small := 128;
            r : REF_TO Small;
        END_VAR
            r := REF(v);
            IF arr[1].7 THEN arr[1].0 := TRUE; END_IF;
            IF s.fld.7 THEN s.fld.1 := TRUE; END_IF;
            IF r^.7 THEN r^.2 := TRUE; END_IF;
            get := arr[1];
            get := get * 1000 + s.fld;
            get := get * 1000 + v;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result, 129_130_132, "bit 7 of 128 is set, so bits 0, 1, 2");
}

/// Exact byte and word values of an LWORD's slices.
///
/// `read_high_slices_of_lword` compares against zero so that one body shape
/// covers both the BOOL of `%X` and the BYTE of `%B` — but every byte of
/// `16#1122334455667788` is non-zero, so a shift off by a whole byte still
/// satisfies it. Its `%X` cases carry the precision for the bit half; these
/// carry it for the sized half, where the value itself is the assertion.
#[rstest]
#[case("l.%B0", "BYTE", 0x88)]
#[case("l.%B3", "BYTE", 0x55)]
#[case("l.%B4", "BYTE", 0x44)]
#[case("l.%B7", "BYTE", 0x11)]
#[case("l.%W0", "WORD", 0x7788)]
#[case("l.%W3", "WORD", 0x1122)]
fn sized_slices_of_an_lword_are_exact(
    mut with_db: db::RootDatabase,
    #[case] access: &str,
    #[case] returns: &str,
    #[case] expected: i32,
) {
    let source = format!(
        r#"
        FUNCTION get : {returns}
        VAR
            l : LWORD := 16#1122334455667788;
        END_VAR
            get := {access};
        END_FUNCTION
    "#
    );
    let result: i32 = run(&mut with_db, &source, "get", ());
    assert_eq!(result, expected, "{access} on 16#1122334455667788");
}

/// A partial write into a SIGNED variable keeps its sign. rk holds a 16-bit
/// INT sign-extended in its 32-bit lane, and the read-modify-write rebuilds
/// only its low 16 bits: without putting it back in its domain, -8 with bit
/// 0 set read back as 65529, and every comparison with zero after it lied.
#[rstest]
#[case::int("INT", "-8", "-7")]
#[case::sint("SINT", "-8", "-7")]
fn a_partial_write_keeps_a_signed_value_signed(
    mut with_db: db::RootDatabase,
    #[case] ty: &str,
    #[case] start: &str,
    #[case] expected: &str,
) {
    let source = format!(
        r#"
        FUNCTION get : BOOL
        VAR
            x : {ty} := {start};
        END_VAR
            x.0 := TRUE;
            get := x = {expected} AND x < 0;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 1, "{ty} {start} with bit 0 set is {expected}");
}

/// A bit write reads the word it rewrites: both go through one address, so a
/// call in the subscript runs once. It ran twice, and the word read from
/// `a[1]` landed in `a[0]`.
#[rstest]
fn a_bit_write_runs_its_subscript_once(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Seq
        VAR_OUTPUT calls : INT; END_VAR
            METHOD Next : INT
                Next := calls;
                calls := calls + 1;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR a : ARRAY[0..3] OF WORD; q : Seq; r : DINT; END_VAR
            a[1] := 16#FF00;
            a[q.Next()].0 := TRUE;
            IF a[0] = 16#0001 THEN r := 1; END_IF;
            IF a[1] = 16#FF00 THEN r := r + 10; END_IF;
            run := r + q.calls * 100;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 111, "a[0] got the bit, a[1] untouched, one call");
}

/// A slice of a FUNCTION's own result: written into a 64-bit result it was
/// the 8-bit value converted twice, an invalid module. The other bits stay,
/// and a signed result keeps its sign.
#[rstest]
fn slices_of_a_result(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION F : LWORD
        VAR_INPUT x : BYTE; END_VAR
            F.%B7 := x;
            F.%X0 := TRUE;
        END_FUNCTION

        FUNCTION G : LWORD
            G := LWORD#16#1122334455667788;
            G.%W1 := WORD#16#ABCD;
        END_FUNCTION

        FUNCTION S : INT
            S := INT#-1;
            S.%B1 := BYTE#16#7F;
        END_FUNCTION

        FUNCTION byte_of_f : LWORD
            byte_of_f := F(BYTE#16#01);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let f: i64 = execute_wasm(&wasm, "byte_of_f", ());
    assert_eq!(f as u64, 0x0100_0000_0000_0001);
    let g: i64 = execute_wasm(&wasm, "G", ());
    assert_eq!(g as u64, 0x1122_3344_ABCD_7788);
    let s: i32 = execute_wasm(&wasm, "S", ());
    assert_eq!(s, 32767, "16#7FFF");
}

/// A slice through a reference is of the value it points to: the place
/// took the slice's type for the pointee's, and a `DWORD` of a `LINT`
/// stopped lowering ("reaches bit 64 of a 32-bit value").
#[rstest]
fn slices_through_a_reference(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR r : REF_TO LINT; l : LINT := -1; high : DWORD; END_VAR
            r := REF(l);
            r^.%D1 := DWORD#0;
            IF l = 4294967295 THEN run := 1; END_IF;
            IF r^.31 THEN run := run + 10; END_IF;
            r^.%B7 := BYTE#16#80;
            high := r^.%D1;
            IF high = DWORD#16#80000000 THEN run := run + 100; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "run", ());
    assert_eq!(
        result, 111,
        "the high half cleared, bit 31 kept, the top byte set"
    );
}

/// A BOOL is its own bit 0. Read, written, and as a FUNCTION's result, it
/// keeps the 0 or 1 a BOOL holds, so it still compares equal to TRUE.
#[rstest]
fn a_bool_is_its_own_bit_zero(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION result : BOOL
            result.0 := TRUE;
        END_FUNCTION

        FUNCTION get : DINT
        VAR
            t : BOOL := TRUE;
            u : BOOL;
            a : BOOL;
            b : BOOL := TRUE;
        END_VAR
            a.0 := TRUE;
            b.%X0 := FALSE;
            get := 0;
            IF t.0 THEN get := get + 1; END_IF;
            IF NOT u.%X0 THEN get := get + 10; END_IF;
            IF a = TRUE THEN get := get + 100; END_IF;
            IF NOT b THEN get := get + 1000; END_IF;
            IF result() = TRUE THEN get := get + 10000; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result, 11111);
}

/// The size letter is caseless, `%b0` is the byte `%B0` is, and it may be
/// left out for a bit, `%12` is `%X12`.
#[rstest]
fn a_lowercase_or_omitted_size_names_the_same_part(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION get : DWORD
        VAR
            d : DWORD := DWORD#16#11223344;
        END_VAR
            d.%w1 := WORD#16#ABCD;
            d.%b0 := BYTE#16#77;
            d.%x8 := FALSE;
            d.%12 := FALSE;
            get := d;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result as u32, 0xABCD_2277);
}

/// A slice of a VAR_IN_OUT reads and writes the caller's variable: the
/// write goes through the reference, the other bits stay.
#[rstest]
fn slices_of_an_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Mark : BOOL
        VAR_IN_OUT w : WORD; END_VAR
            Mark := w.15;
            w.%B1 := BYTE#16#AB;
            w.0 := TRUE;
        END_FUNCTION

        FUNCTION get : WORD
        VAR w : WORD := WORD#16#8010; was : BOOL; END_VAR
            was := Mark(w);
            IF was THEN w.%X1 := TRUE; END_IF;
            get := w;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(
        result, 0xAB13,
        "byte 1 replaced, bits 0 and 1 set, bit 4 kept"
    );
}

/// Inside a METHOD, a slice of the instance's state and of the METHOD's own
/// result, as of a FUNCTION's.
#[rstest]
fn slices_in_a_method(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Reg
        VAR_OUTPUT flags : BYTE; END_VAR
            METHOD Pack : DWORD
            VAR_INPUT hi : WORD; lo : WORD; END_VAR
                Pack.%W1 := hi;
                Pack.%W0 := lo;
                flags.3 := TRUE;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION get : DWORD
        VAR r : Reg; d : DWORD; END_VAR
            d := r.Pack(hi := WORD#16#1234, lo := WORD#16#5678);
            IF r.flags = BYTE#2#0000_1000 THEN get := d; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = run(&mut with_db, source, "get", ());
    assert_eq!(result as u32, 0x1234_5678);
}
