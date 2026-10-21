// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A TIME, an LTIME, a TOD and a DT compared exactly with a constant, a
/// literal or a CONSTANT that holds one.
#[rstest]
fn a_time_compared_with_a_constant(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Timed
VAR_INPUT
    et : TIME;
    lt : LTIME;
    now : TOD;
    stamp : DT;
END_VAR
VAR_OUTPUT done : BOOL; END_VAR
VAR CONSTANT LIMIT : TIME := T#2S; END_VAR
    done := et = T#5S;
    done := lt <> LTIME#1MS;
    done := TOD#12:00:00 = now;
    done := stamp = DT#2026-01-01-12:00:00;
    done := et = LIMIT;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "time-equality"), @r"
    [L0131] Warning: exact comparison of time values
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     done := et = T#5S;
        |             ^^^^|^^^^
        |                 `------ TIME values compared with '='
        |
        | Help: write 'et >= T#5S'
        |
        | Note 1: a time read once per scan can step over the exact value
        |
        | Note 2: lint rule: time-equality
    ----'
    [L0131] Warning: exact comparison of time values
        ,-[ file:///test0.st:12:13 ]
        |
     12 |     done := lt <> LTIME#1MS;
        |             ^^^^^^^|^^^^^^^
        |                    `--------- LTIME values compared with '<>'
        |
        | Help: write 'lt < LTIME#1MS'
        |
        | Note 1: a time read once per scan can step over the exact value
        |
        | Note 2: lint rule: time-equality
    ----'
    [L0131] Warning: exact comparison of time values
        ,-[ file:///test0.st:13:13 ]
        |
     13 |     done := TOD#12:00:00 = now;
        |             ^^^^^^^^^|^^^^^^^^
        |                      `---------- TOD values compared with '='
        |
        | Help: write 'now >= TOD#12:00:00'
        |
        | Note 1: a time read once per scan can step over the exact value
        |
        | Note 2: lint rule: time-equality
    ----'
    [L0131] Warning: exact comparison of time values
        ,-[ file:///test0.st:14:13 ]
        |
     14 |     done := stamp = DT#2026-01-01-12:00:00;
        |             ^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^
        |                            `---------------- DT values compared with '='
        |
        | Help: write 'stamp >= DT#2026-01-01-12:00:00'
        |
        | Note 1: a time read once per scan can step over the exact value
        |
        | Note 2: lint rule: time-equality
    ----'
    [L0131] Warning: exact comparison of time values
        ,-[ file:///test0.st:15:13 ]
        |
     15 |     done := et = LIMIT;
        |             ^^^^^|^^^^
        |                  `------ TIME values compared with '='
        |
        | Help: write 'et >= LIMIT'
        |
        | Note 1: a time read once per scan can step over the exact value
        |
        | Note 2: lint rule: time-equality
    ----'
    ");
}

/// Two variables, a zero duration, an ordering, a DATE and two constants
/// are not reported.
#[rstest]
fn what_is_not_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Settings
VAR_INPUT
    pt : TIME;
    et : TIME;
    day : DATE;
END_VAR
VAR
    last_pt : TIME;
    changed, done : BOOL;
END_VAR
VAR CONSTANT ZERO : TIME := T#0S; END_VAR
    changed := pt <> last_pt;
    last_pt := pt;
    done := et = T#0S;
    done := et <> ZERO;
    done := et >= T#5S;
    done := day = D#2026-12-25;
    done := T#1S = T#1000MS;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "time-equality"), @r"");
}
