// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v&` is the AND of its arguments: logic on BOOL, bitwise on an integer.

use db::RootDatabase;
use rstest::rstest;

use super::{Int, assert_fold, fails, holds, with_db};

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
fn on_each_integer(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let t = Int::of(ty);
    assert_fold(
        &mut with_db,
        ty,
        ty,
        "&",
        &[
            (t.args(&[12]), t.lit(12)),
            (t.args(&[12, 10]), t.lit(8)),
            (t.args(&[12, 10, 1]), t.lit(0)),
            (t.args(&[t.all(), t.top()]), t.lit(t.top())),
        ],
    );
}

#[rstest]
fn on_bool(mut with_db: RootDatabase) {
    assert_fold(
        &mut with_db,
        "BOOL",
        "BOOL",
        "&",
        &[
            holds("TRUE".to_string()),
            holds("TRUE, TRUE, TRUE".to_string()),
            fails("TRUE, FALSE, TRUE".to_string()),
            fails("FALSE, TRUE".to_string()),
        ],
    );
}
