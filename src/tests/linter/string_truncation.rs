// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A longer STRING into a shorter one: assigned, a field included, passed
/// to an input, or bound from an output. A plain STRING holds 80 bytes.
#[rstest]
fn longer_string_into_shorter(mut with_db: RootDatabase) {
    let source = r#"
TYPE Tag : STRUCT label : STRING[8]; END_STRUCT; END_TYPE

FUNCTION show
VAR_INPUT text : STRING[10]; END_VAR
END_FUNCTION

FUNCTION read
VAR_OUTPUT text : STRING[40]; END_VAR
    text := 'abc';
END_FUNCTION

FUNCTION test : INT
VAR
    long : STRING[20];
    plain : STRING;
    short : STRING[5];
    tag : Tag;
END_VAR
    short := long;
    tag.label := plain;
    show(text := long);
    read(text => short);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "string-truncation"), @r"
    [L0127] Warning: STRING into a shorter STRING
        ,-[ file:///test0.st:22:18 ]
        |
     22 |     show(text := long);
        |                  ^^|^
        |                    `--- a STRING[20] is passed to the STRING[10] 'text'
        |
        | Help: declare 'text' as STRING[20]
        |
        | Note 1: a text longer than 10 bytes is cut
        |
        | Note 2: lint rule: string-truncation
    ----'
    [L0127] Warning: STRING into a shorter STRING
        ,-[ file:///test0.st:23:18 ]
        |
     23 |     read(text => short);
        |                  ^^|^^
        |                    `---- the STRING[40] 'text' is bound to a STRING[5]
        |
        | Help: declare 'short' as STRING[40]
        |
        | Note 1: a text longer than 5 bytes is cut
        |
        | Note 2: lint rule: string-truncation
    ----'
    [L0127] Warning: STRING into a shorter STRING
        ,-[ file:///test0.st:20:14 ]
        |
     20 |     short := long;
        |              ^^|^
        |                `--- a STRING[20] is assigned to a STRING[5]
        |
        | Help: declare 'short' as STRING[20]
        |
        | Note 1: a text longer than 5 bytes is cut
        |
        | Note 2: lint rule: string-truncation
    ----'
    [L0127] Warning: STRING into a shorter STRING
        ,-[ file:///test0.st:21:18 ]
        |
     21 |     tag.label := plain;
        |                  ^^|^^
        |                    `---- a STRING[80] is assigned to a STRING[8]
        |
        | Help: declare 'tag.label' as STRING[80]
        |
        | Note 1: a text longer than 8 bytes is cut
        |
        | Note 2: lint rule: string-truncation
    ----'
    ");
}

/// Nothing is cut: a STRING into one as long or longer, an in-out (it takes
/// the caller's capacity), a literal (E0314 measures it), a call's result.
#[rstest]
fn nothing_cut_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION edit
VAR_IN_OUT text : STRING[10]; END_VAR
END_FUNCTION

FUNCTION make : STRING[30]
    make := 'abc';
END_FUNCTION

FUNCTION test : INT
VAR
    a : STRING[20];
    b : STRING[20];
    wide : STRING[40];
    short : STRING[5];
END_VAR
    a := b;
    wide := a;
    edit(text := short);
    short := 'abc';
    short := make();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "string-truncation"), @r"");
}
