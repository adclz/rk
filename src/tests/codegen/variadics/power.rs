// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `...v**` raises from the left: `2.0, 3.0, 2.0` is 64.0, where from the right it
//! would be 512.0. It takes REAL and LREAL.

use db::RootDatabase;
use rstest::rstest;

use super::{assert_fold, with_db};

#[rstest]
#[case::real("REAL")]
#[case::lreal("LREAL")]
fn on_each_float(mut with_db: RootDatabase, #[case] ty: &str) {
    let l = |value: &str| format!("{ty}#{value}");
    assert_fold(
        &mut with_db,
        ty,
        ty,
        "**",
        &[
            (l("2.0"), l("2.0")),
            (format!("{}, {}", l("2.0"), l("3.0")), l("8.0")),
            (
                format!("{}, {}, {}", l("2.0"), l("3.0"), l("2.0")),
                l("64.0"),
            ),
            (format!("{}, {}", l("4.0"), l("0.5")), l("2.0")),
            (format!("{}, {}", l("2.0"), l("-1.0")), l("0.5")),
        ],
    );
}
