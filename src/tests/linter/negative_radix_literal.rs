// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! L0123: an untyped radix literal with its top bit set, where a signed
//! integer is expected, is negative: `16#80` is -128 in a SINT.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// In a TYPE's default, a field's, an initializer, a CONSTANT, a statement,
/// a product, a call argument, a comparison, a CASE label and under a binary
/// minus, at every signed width and radix: each literal reads negative, and
/// the fix writes its type in front.
#[rstest]
fn a_negative_radix_literal_is_reported_wherever_it_is_written(mut with_db: RootDatabase) {
    let source = r#"
TYPE Mask : SINT := 16#F0; END_TYPE
TYPE Pair : STRUCT lo : SINT := 16#FF; END_STRUCT; END_TYPE

FUNCTION Take : INT
VAR_INPUT v : SINT; END_VAR
    Take := v;
END_FUNCTION

FUNCTION Reported : INT
VAR
    t : SINT := 16#80;
    s : SINT;
    i : INT;
    d : DINT;
    l : LINT;
END_VAR
VAR CONSTANT
    K : SINT := 2#1000_0000;
END_VAR
    s := 8#200;
    s := t * 16#FF;
    i := 16#8000;
    d := 16#8000_0000;
    l := 16#8000_0000_0000_0000;
    Reported := Take(v := 16#C0);
    IF s = 16#FF THEN i := 0; END_IF;
    CASE s OF
        16#FE : i := 1;
    END_CASE;
    i := i - (16#FFFF);
    s := K;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negative-radix-literal"), @r"
    [L0123] Warning: negative radix literal
       ,-[ file:///test0.st:2:21 ]
       |
     2 | TYPE Mask : SINT := 16#F0; END_TYPE
       |                     ^^|^^
       |                       `---- '16#F0' is the SINT -16
       |
       | Help: write SINT#16#F0
       |
       | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
       |
       | Note 2: lint rule: negative-radix-literal
    ---'
    [L0123] Warning: negative radix literal
       ,-[ file:///test0.st:3:33 ]
       |
     3 | TYPE Pair : STRUCT lo : SINT := 16#FF; END_STRUCT; END_TYPE
       |                                 ^^|^^
       |                                   `---- '16#FF' is the SINT -1
       |
       | Help: write SINT#16#FF
       |
       | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
       |
       | Note 2: lint rule: negative-radix-literal
    ---'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:12:17 ]
        |
     12 |     t : SINT := 16#80;
        |                 ^^|^^
        |                   `---- '16#80' is the SINT -128
        |
        | Help: write SINT#16#80
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:19:17 ]
        |
     19 |     K : SINT := 2#1000_0000;
        |                 ^^^^^|^^^^^
        |                      `------- '2#1000_0000' is the SINT -128
        |
        | Help: write SINT#2#1000_0000
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:21:10 ]
        |
     21 |     s := 8#200;
        |          ^^|^^
        |            `---- '8#200' is the SINT -128
        |
        | Help: write SINT#8#200
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:22:14 ]
        |
     22 |     s := t * 16#FF;
        |              ^^|^^
        |                `---- '16#FF' is the SINT -1
        |
        | Help: write SINT#16#FF
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:23:10 ]
        |
     23 |     i := 16#8000;
        |          ^^^|^^^
        |             `----- '16#8000' is the INT -32768
        |
        | Help: write INT#16#8000
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:24:10 ]
        |
     24 |     d := 16#8000_0000;
        |          ^^^^^^|^^^^^
        |                `------- '16#8000_0000' is the DINT -2147483648
        |
        | Help: write DINT#16#8000_0000
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:25:10 ]
        |
     25 |     l := 16#8000_0000_0000_0000;
        |          ^^^^^^^^^^^|^^^^^^^^^^
        |                     `------------ '16#8000_0000_0000_0000' is the LINT -9223372036854775808
        |
        | Help: write LINT#16#8000_0000_0000_0000
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:26:27 ]
        |
     26 |     Reported := Take(v := 16#C0);
        |                           ^^|^^
        |                             `---- '16#C0' is the SINT -64
        |
        | Help: write SINT#16#C0
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:27:12 ]
        |
     27 |     IF s = 16#FF THEN i := 0; END_IF;
        |            ^^|^^
        |              `---- '16#FF' is the SINT -1
        |
        | Help: write SINT#16#FF
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:29:9 ]
        |
     29 |         16#FE : i := 1;
        |         ^^|^^
        |           `---- '16#FE' is the SINT -2
        |
        | Help: write SINT#16#FE
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    [L0123] Warning: negative radix literal
        ,-[ file:///test0.st:31:15 ]
        |
     31 |     i := i - (16#FFFF);
        |               ^^^|^^^
        |                  `----- '16#FFFF' is the INT -1
        |
        | Help: write INT#16#FFFF
        |
        | Note 1: a radix literal is a bit pattern, with its top bit the sign of a signed type
        |
        | Note 2: lint rule: negative-radix-literal
    ----'
    ");
}

/// A typed literal says the pattern is meant. A wider type, an unsigned one
/// or a clear top bit keeps the number. A minus makes the literal a number,
/// and AND, OR and XOR read its bits.
#[rstest]
fn a_typed_literal_a_number_and_a_mask_are_not_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Quiet : INT
VAR
    s : SINT := SINT#16#80;
    i : INT := 16#80;
    b : BYTE := 16#FF;
    u : USINT := 16#FF;
    m : SINT := 16#7F;
    d : DINT := 16#7FFF_FFFF;
END_VAR
    s := -(16#80);
    s := -(+(16#80));
    s := -128;
    i := i AND 16#FF00;
    i := (16#8000) OR i;
    i := i XOR 16#FFFF;
    Quiet := s + i;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negative-radix-literal"), @r"");
}
