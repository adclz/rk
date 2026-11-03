// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v*` multiplies its arguments, and wraps at the width of the type.

use db::RootDatabase;
use rstest::rstest;

use super::{Int, assert_fold, with_db};

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
        "*",
        &[
            (t.args(&[5]), t.lit(5)),
            (t.args(&[2, 3]), t.lit(6)),
            (t.args(&[2, 3, 4]), t.lit(24)),
            (t.args(&[t.max(), 2]), t.lit(t.wrap(t.max() * 2))),
        ],
    );
}

#[rstest]
#[case::real("REAL", "REAL#1.5", "REAL#2.0", "REAL#3.0", "REAL#3.0", "REAL#9.0")]
#[case::lreal(
    "LREAL",
    "LREAL#1.5",
    "LREAL#2.0",
    "LREAL#3.0",
    "LREAL#3.0",
    "LREAL#9.0"
)]
fn on_each_float(
    mut with_db: RootDatabase,
    #[case] ty: &str,
    #[case] a: &str,
    #[case] b: &str,
    #[case] c: &str,
    #[case] ab: &str,
    #[case] abc: &str,
) {
    assert_fold(
        &mut with_db,
        ty,
        ty,
        "*",
        &[
            (a.to_string(), a.to_string()),
            (format!("{a}, {b}"), ab.to_string()),
            (format!("{a}, {b}, {c}"), abc.to_string()),
        ],
    );
}
