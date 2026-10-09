// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The BCD conversions of `Std.Bytes` raise on what has no BCD form: a
//! digit above 9, a number with more digits than the bit string holds. The
//! stdlib's own tests cover the values. A raise is what they cannot check.

use rstest::rstest;

use super::with_db;

/// The message `call` raises, in a workspace with `Std.Bytes`, which uses
/// no other namespace. Its tests are a library's, and not lowered.
fn raised(db: &mut db::RootDatabase, call: &str) -> String {
    let source = format!(
        "USING Std.Bytes;
FUNCTION run : DINT
VAR w : WORD; n : UINT; END_VAR
    {call};
    run := 0;
END_FUNCTION
"
    );
    let wasm = crate::tests::codegen::compile_with_libraries(
        db,
        &[include_str!("../../../stdlib/Bytes.st")],
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
