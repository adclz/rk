// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The `retain-map` section: where each retained range is in the retain
//! band, how large the band is, and the layout hash a host compares before
//! it gives the saved bytes back.

use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use rstest::*;

/// A program with two retained variables, declared in the order given.
fn program(first: &str, second: &str) -> String {
    format!(
        r#"
        PROGRAM P
        VAR RETAIN
            {first}
            {second}
        END_VAR
            count := count + 1;
            hours := 40;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#
    )
}

/// The MIR of `source`, in a database of its own: each is a whole workspace.
fn lowered(source: &str) -> mir::MirModule {
    let mut db = db::RootDatabase::default();
    compile_to_mir_and_wasm(&mut db, source).0
}

/// Each range says where it is in the retain band, which is its place in
/// the list `save-retain` returns, and the map says how large the band is.
#[rstest]
fn each_range_says_where_it_is_in_the_band(#[allow(unused)] with_db: db::RootDatabase) {
    let mir = lowered(&program("count : DINT;", "hours : DINT;"));
    let map = &mir.retain_map;
    assert_eq!(map.version, debug_format::RETAIN_MAP_VERSION);
    assert_eq!(map.size, Some(mir.retain_size));
    assert_eq!(map.ranges.len(), 2);
    for range in &map.ranges {
        assert_eq!(
            range.offset,
            Some(range.addr - mir.retain_base),
            "{}",
            range.path
        );
    }
}

/// The same retained variables in another order are another layout. The
/// hash covered their paths, sizes and types only, so a host that gave the
/// saved bytes back because it matched would have given each value to the
/// other variable.
#[rstest]
fn the_same_variables_in_another_order_are_another_layout(
    #[allow(unused)] with_db: db::RootDatabase,
) {
    let declared = lowered(&program("count : DINT;", "hours : DINT;"));
    let swapped = lowered(&program("hours : DINT;", "count : DINT;"));
    let offset = |mir: &mir::MirModule, path: &str| {
        mir.retain_map
            .ranges
            .iter()
            .find(|r| r.path == path)
            .and_then(|r| r.offset)
    };
    assert_eq!(offset(&declared, "P1.count"), Some(0));
    assert_eq!(offset(&swapped, "P1.count"), Some(4));
    assert_ne!(
        declared.retain_map.layout_hash,
        swapped.retain_map.layout_hash
    );
}
