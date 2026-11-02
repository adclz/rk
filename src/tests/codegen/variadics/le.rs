// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v<=` holds when each argument is at most the next, every pair of them. One
//! argument has no pair to test, and holds.

use db::RootDatabase;
use rstest::rstest;

use super::{assert_fold, fails, holds, ordered, with_db};

#[rstest]
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
        "BOOL",
        "<=",
        &[
            holds(low.clone()),
            holds(format!("{low}, {high}")),
            holds(format!("{mid}, {mid}")),
            holds(format!("{low}, {mid}, {mid}, {high}")),
            fails(format!("{high}, {low}")),
            fails(format!("{mid}, {low}, {high}")),
            fails(format!("{low}, {high}, {mid}")),
        ],
    );
}

#[rstest]
fn on_bool(mut with_db: RootDatabase) {
    assert_fold(
        &mut with_db,
        "BOOL",
        "BOOL",
        "<=",
        &[
            holds("FALSE, TRUE".to_string()),
            holds("TRUE, TRUE".to_string()),
            fails("TRUE, FALSE".to_string()),
        ],
    );
}

/// A NaN compares false, except by `<>`.
#[rstest]
fn on_a_nan(mut with_db: RootDatabase) {
    assert_fold(
        &mut with_db,
        "REAL",
        "BOOL",
        "<=",
        &[
            fails("Nan(), REAL#1.0".to_string()),
            fails("REAL#1.0, Nan()".to_string()),
            fails("Nan(), Nan()".to_string()),
        ],
    );
}
