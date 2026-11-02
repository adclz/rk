// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v-` subtracts from the left, and wraps at the width of the type: `10, 3, 2`
//! is 5, where from the right it would be 9.

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
        "-",
        &[
            (t.args(&[10]), t.lit(10)),
            (t.args(&[10, 3]), t.lit(7)),
            (t.args(&[10, 3, 2]), t.lit(5)),
            (t.args(&[t.min(), 1]), t.lit(t.max())),
        ],
    );
}

#[rstest]
#[case::real("REAL", "REAL#7.5", "REAL#2.5", "REAL#1.5", "REAL#5.0", "REAL#3.5")]
#[case::lreal(
    "LREAL",
    "LREAL#7.5",
    "LREAL#2.5",
    "LREAL#1.5",
    "LREAL#5.0",
    "LREAL#3.5"
)]
#[case::time("TIME", "T#6s", "T#2s", "T#1s", "T#4s", "T#3s")]
#[case::ltime("LTIME", "LT#6s", "LT#2s", "LT#1s", "LT#4s", "LT#3s")]
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
        "-",
        &[
            (a.to_string(), a.to_string()),
            (format!("{a}, {b}"), ab.to_string()),
            (format!("{a}, {b}, {c}"), abc.to_string()),
        ],
    );
}
