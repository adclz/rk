// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v MAX` gives the largest argument. Each step keeps the value so far when `>=`
//! holds of it and the next argument, and takes that argument otherwise.

use db::RootDatabase;
use rstest::rstest;

use super::{assert_fold, execute_wasm, ordered, run, with_db};

#[rstest]
#[case::bool("BOOL")]
#[case::sint("SINT")]
#[case::int("INT")]
#[case::dint("DINT")]
#[case::lint("LINT")]
#[case::usint("USINT")]
#[case::uint("UINT")]
#[case::udint("UDINT")]
#[case::ulint("ULINT")]
#[case::byte("BYTE")]
#[case::word("WORD")]
#[case::dword("DWORD")]
#[case::lword("LWORD")]
#[case::real("REAL")]
#[case::lreal("LREAL")]
#[case::time("TIME")]
#[case::ltime("LTIME")]
#[case::date("DATE")]
#[case::ldate("LDATE")]
#[case::tod("TOD")]
#[case::ltod("LTOD")]
#[case::dt("DT")]
#[case::ldt("LDT")]
#[case::char_("CHAR")]
fn on_each_type(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let [low, mid, high] = ordered(ty);
    assert_fold(
        &mut with_db,
        ty,
        ty,
        " MAX",
        &[
            (mid.clone(), mid.clone()),
            (format!("{high}, {mid}, {low}"), high.clone()),
            (format!("{mid}, {high}, {low}"), high.clone()),
            (format!("{low}, {mid}, {high}"), high.clone()),
            (format!("{low}, {high}"), high.clone()),
            (format!("{high}, {low}"), high.clone()),
            (format!("{low}, {mid}, {mid}, {high}"), high.clone()),
        ],
    );
}

/// A NaN compares false, so it gives way when it is the value so far, and takes
/// over when it is the next argument.
#[rstest]
#[case::nan_first("F(n, 1.0, 2.0) = 2.0")]
#[case::nan_between("F(1.0, n, 2.0) = 2.0")]
#[case::nan_last("F(2.0, 1.0, n) <> F(2.0, 1.0, n)")]
fn on_a_nan(mut with_db: RootDatabase, #[case] holds: &str) {
    let source = format!(
        r#"
        FUNCTION F : REAL
        VAR_INPUT v : REAL...; END_VAR
            F := ...v MAX;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR zero : REAL := 0.0; n : REAL; END_VAR
            n := zero / zero;
            test := {holds};
        END_FUNCTION
    "#
    );
    let result: i32 = run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{holds}");
}

/// A step reads the value so far once. Written as a nested `select` it would read
/// it twice, and forty arguments would take 2^40 loads.
#[rstest]
fn on_many_arguments_stays_small(mut with_db: RootDatabase) {
    let arguments = (0..40)
        .map(|i| (100 - i).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"
        FUNCTION F : INT
        VAR_INPUT v : INT...; END_VAR
            F := ...v MAX;
        END_FUNCTION

        FUNCTION test : INT
            test := F({arguments});
        END_FUNCTION
    "#
    );
    let wasm = crate::tests::codegen::compile_to_wasm(&mut with_db, &source);
    assert!(wasm.len() < 30_000, "{} bytes", wasm.len());
    let result: i32 = execute_wasm(&wasm, "test", ());
    assert_eq!(result, 100);
}
