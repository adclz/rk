//! Partial (bit / byte / word) variable access — `b.1`, `w.%X3`, `d.%B2`.
//!
//! IEC 61131-3 §6.5.5. Before these landed, MIR ignored the `multibits` part
//! entirely: a read loaded the *whole* variable and merely relabelled it with
//! the slice's type (`b.1` on `BYTE#5` yielded `TRUE` because 5 ≠ 0), and a
//! write stored through the slice onto the whole variable, wiping its other
//! bits. Every test here therefore checks a value that only comes out right if
//! the shift and the mask are both applied.

use crate::tests::codegen::{compile_to_wasm, execute_wasm, with_db};
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "count_low_nibble", ());
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "set", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "clear", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "build", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "patch", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "widen", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = execute_wasm(&wasm, "set", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 0x12, "byte 1 of 16#1234");
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
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
