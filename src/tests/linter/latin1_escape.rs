// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! L0121: a `$hh` escape of $80 or more in a STRING literal is one byte, not
//! a Latin-1 character, so the literal is no UTF-8 text.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// In a statement, a call argument, an initializer, a typed literal and a
/// TYPE's default: each literal is read, and the fix offers its Latin-1
/// reading as UTF-8.
#[rstest]
fn a_latin1_escape_is_reported_wherever_it_is_written(mut with_db: RootDatabase) {
    let source = r#"
TYPE Place : STRING := 'M$FCnchen'; END_TYPE

FUNCTION Show : INT
VAR_INPUT s : STRING; END_VAR
    Show := 0;
END_FUNCTION

FUNCTION Caller : INT
VAR
    greeting : STRING := 'caf$E9';
    s : STRING;
END_VAR
    s := 'na$EFve';
    s := STRING#'r$E9sum$E9';
    Caller := Show('$A3100');
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "latin1-escape"), @r"
    [L0121] Warning: STRING with a Latin-1 escape
       ,-[ file:///test0.st:2:24 ]
       |
     2 | TYPE Place : STRING := 'M$FCnchen'; END_TYPE
       |                        ^^^^^|^^^^^
       |                             `------- `$FC` is one byte, not 'ü': a STRING is UTF-8, where 'ü' is `$C3$BC`
       |
       | Help: write 'München'
       |
       | Note: lint rule: latin1-escape
    ---'
    [L0121] Warning: STRING with a Latin-1 escape
        ,-[ file:///test0.st:11:26 ]
        |
     11 |     greeting : STRING := 'caf$E9';
        |                          ^^^^|^^^
        |                              `----- `$E9` is one byte, not 'é': a STRING is UTF-8, where 'é' is `$C3$A9`
        |
        | Help: write 'café'
        |
        | Note: lint rule: latin1-escape
    ----'
    [L0121] Warning: STRING with a Latin-1 escape
        ,-[ file:///test0.st:14:10 ]
        |
     14 |     s := 'na$EFve';
        |          ^^^^|^^^^
        |              `------ `$EF` is one byte, not 'ï': a STRING is UTF-8, where 'ï' is `$C3$AF`
        |
        | Help: write 'naïve'
        |
        | Note: lint rule: latin1-escape
    ----'
    [L0121] Warning: STRING with a Latin-1 escape
        ,-[ file:///test0.st:15:10 ]
        |
     15 |     s := STRING#'r$E9sum$E9';
        |          ^^^^^^^^^|^^^^^^^^^
        |                   `----------- `$E9` is one byte, not 'é': a STRING is UTF-8, where 'é' is `$C3$A9`
        |
        | Help: write STRING#'résumé'
        |
        | Note: lint rule: latin1-escape
    ----'
    [L0121] Warning: STRING with a Latin-1 escape
        ,-[ file:///test0.st:16:20 ]
        |
     16 |     Caller := Show('$A3100');
        |                    ^^^^|^^^
        |                        `----- `$A3` is one byte, not '£': a STRING is UTF-8, where '£' is `$C2$A3`
        |
        | Help: write '£100'
        |
        | Note: lint rule: latin1-escape
    ----'
    ");
}

/// UTF-8 written as itself or as its bytes, ASCII escapes, and a CHAR, which
/// is a code point: `$E9` is `é` there.
#[rstest]
fn utf8_text_and_a_char_are_not_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Fine : INT
VAR
    a : STRING := 'café';
    b : STRING := 'caf$C3$A9';
    c : STRING := 'line$Nnext$T$$5';
    d : CHAR := '$E9';
    e : CHAR := CHAR#'$E9';
END_VAR
    d := '$FC';
    Fine := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "latin1-escape"), @r"");
}
