// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v/` divides from the left: `100, 5, 2` is 10, where from the right it would
//! be 50. An integer division truncates toward zero, and wraps where the
//! quotient does not fit.

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
        "/",
        &[
            (t.args(&[100]), t.lit(100)),
            (t.args(&[100, 5]), t.lit(20)),
            (t.args(&[100, 5, 2]), t.lit(10)),
            (t.args(&[t.max(), 2]), t.lit(t.max() / 2)),
        ],
    );
}

#[rstest]
#[case::sint("SINT")]
#[case::int("INT")]
#[case::dint("DINT")]
#[case::lint("LINT")]
fn on_each_signed_integer(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let t = Int::of(ty);
    assert_fold(
        &mut with_db,
        ty,
        ty,
        "/",
        &[
            (t.args(&[-7, 2]), t.lit(-3)),
            (t.args(&[7, -2]), t.lit(-3)),
            (t.args(&[t.min(), -1]), t.lit(t.min())),
        ],
    );
}

#[rstest]
#[case::real("REAL", "REAL#9.0", "REAL#3.0", "REAL#1.5", "REAL#3.0", "REAL#2.0")]
#[case::lreal(
    "LREAL",
    "LREAL#9.0",
    "LREAL#3.0",
    "LREAL#1.5",
    "LREAL#3.0",
    "LREAL#2.0"
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
        "/",
        &[
            (a.to_string(), a.to_string()),
            (format!("{a}, {b}"), ab.to_string()),
            (format!("{a}, {b}, {c}"), abc.to_string()),
        ],
    );
}

/// An integer divided by zero raises, as the operator does.
#[rstest]
fn by_zero_faults(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION F : DINT
        VAR_INPUT v : DINT...; END_VAR
            F := ...v/;
        END_FUNCTION

        FUNCTION run : DINT
        VAR zero : DINT; END_VAR
            run := F(1, zero);
        END_FUNCTION
    "#;
    let fault =
        crate::tests::codegen::traps::expect_fault(&mut with_db, source, "1 / 0 must fault");
    assert_eq!(fault, "division by zero");
}
