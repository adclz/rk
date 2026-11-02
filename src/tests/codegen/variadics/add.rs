// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v+` adds its arguments, and wraps at the width of the type.

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
        "+",
        &[
            (t.args(&[1]), t.lit(1)),
            (t.args(&[1, 2]), t.lit(3)),
            (t.args(&[1, 2, 3]), t.lit(6)),
            (t.args(&[t.max(), 1]), t.lit(t.min())),
        ],
    );
}

#[rstest]
#[case::real("REAL", "REAL#1.5", "REAL#2.5", "REAL#3.5", "REAL#4.0", "REAL#7.5")]
#[case::lreal(
    "LREAL",
    "LREAL#1.5",
    "LREAL#2.5",
    "LREAL#3.5",
    "LREAL#4.0",
    "LREAL#7.5"
)]
#[case::time("TIME", "T#1s", "T#2s", "T#3s", "T#3s", "T#6s")]
#[case::ltime("LTIME", "LT#1s", "LT#2s", "LT#3s", "LT#3s", "LT#6s")]
fn on_each_other_type(
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
        "+",
        &[
            (a.to_string(), a.to_string()),
            (format!("{a}, {b}"), ab.to_string()),
            (format!("{a}, {b}, {c}"), abc.to_string()),
        ],
    );
}
