// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What the byte functions refuse at run time: a BCD conversion of
//! `Std.Bytes` raises on what has no BCD form, a digit above 9 or a number
//! with more digits than the bit string holds, and a buffer read or write of
//! `Std.Arrays` faults where its bytes run past the array. The stdlib's own
//! tests cover the values. A raise is what they cannot check.

use rstest::rstest;

use super::with_db;

/// The message `call` raises, in a workspace with `Std.Bytes` and
/// `Std.Arrays`, which use no other namespace. Their tests are a library's,
/// and not lowered.
fn raised(db: &mut db::RootDatabase, call: &str) -> String {
    let source = format!(
        "USING Std.Bytes;
USING Std.Arrays;
FUNCTION run : DINT
VAR w : WORD; n : UINT; d : DWORD; buf : ARRAY[0..3] OF BYTE; END_VAR
    {call};
    run := 0;
END_FUNCTION
"
    );
    let wasm = crate::tests::codegen::compile_with_libraries(
        db,
        &[
            include_str!("../../../stdlib/Bytes.st"),
            include_str!("../../../stdlib/Arrays.st"),
        ],
        &source,
    );
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
    let run = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = run.call(&mut store, ()).expect_err(call);
    super::fault_message(&mut store, memory, err)
}

#[rstest]
#[case::a_digit_above_9("n := WORD_BCD_TO_UINT(WORD#16#12A4)", "invalid BCD digit")]
#[case::the_top_digit("n := BCD_TO_UINT(WORD#16#F000)", "invalid BCD digit")]
#[case::more_digits_than_a_word("w := UINT_TO_BCD_WORD(UINT#10000)", "value too large for BCD")]
#[case::by_the_result("w := TO_BCD_WORD(UINT#65535)", "value too large for BCD")]
fn what_has_no_bcd_form_raises(
    mut with_db: db::RootDatabase,
    #[case] call: &str,
    #[case] message: &str,
) {
    let raised = raised(&mut with_db, call);
    assert!(raised.contains(message), "{call}: got {raised}");
}

/// A buffer position is a subscript of the array: four bytes from 1 run past
/// `ARRAY[0..3]`, and so does a position below its lower bound.
#[rstest]
#[case::read_past_the_end("d := GET_DWORD_BE(buf, 1)")]
#[case::write_past_the_end("PUT_DWORD_LE(buf, 1, DWORD#16#12345678)")]
#[case::below_the_start("w := GET_WORD_LE(buf, -1)")]
fn a_position_past_the_array_faults(mut with_db: db::RootDatabase, #[case] call: &str) {
    let raised = raised(&mut with_db, call);
    assert!(
        raised.contains("array index out of bounds"),
        "{call}: got {raised}"
    );
}
