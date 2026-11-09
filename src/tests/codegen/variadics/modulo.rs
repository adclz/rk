// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v%` takes the remainder from the left: `100, 7, 3` is 2, where from the right
//! it would be 0. The sign is the dividend's.

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
fn on_each_integer(mut with_db: RootDatabase, #[case] ty: &'static str) {
    let t = Int::of(ty);
    assert_fold(
        &mut with_db,
        ty,
        ty,
        "%",
        &[
            (t.args(&[17]), t.lit(17)),
            (t.args(&[17, 5]), t.lit(2)),
            (t.args(&[100, 7, 3]), t.lit(2)),
            (t.args(&[t.max(), 7]), t.lit(t.max() % 7)),
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
        "%",
        &[
            (t.args(&[-7, 3]), t.lit(-1)),
            (t.args(&[7, -3]), t.lit(1)),
            (t.args(&[t.min(), -1]), t.lit(0)),
        ],
    );
}

/// A remainder by zero raises, as the operator does.
#[rstest]
fn by_zero_faults(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION F : DINT
        VAR_INPUT v : DINT...; END_VAR
            F := ...v%;
        END_FUNCTION

        FUNCTION run : DINT
        VAR zero : DINT; END_VAR
            run := F(1, zero);
        END_FUNCTION
    "#;
    let fault =
        crate::tests::codegen::traps::expect_fault(&mut with_db, source, "1 % 0 must fault");
    assert_eq!(fault, "division by zero");
}
