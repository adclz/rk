// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/*
    Multiple invalid literal values tests for different types
*/

#[rstest]
fn invalid_bool_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: BOOL := 256;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: BOOL := 256;
       |                       ^|^
       |                        `--- invalid boolean literal
       |
       | Note: BOOL is TRUE or FALSE
    ---'
    ");
}

#[rstest]
fn invalid_u8_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: USINT := -1;
        test2: BYTE := 256;
        test3: USINT := 16#FFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: USINT := -1;
       |                         ^|
       |                          `-- the value is negative and USINT is unsigned
       |
       | Help: use SINT, or drop the sign
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: BYTE := 256;
       |                        ^|^
       |                         `--- the value does not fit in BYTE
       |
       | Note: BYTE holds 0 to 255
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: USINT := 16#FFFF;
       |                         ^^^|^^^
       |                            `----- the value does not fit in USINT
       |
       | Note: USINT holds 0 to 255
    ---'
    ");
}

#[rstest]
fn invalid_u16_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: UINT := -1;
        test2: WORD := 65536;
        test3: UINT := 16#FFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: UINT := -1;
       |                        ^|
       |                         `-- the value is negative and UINT is unsigned
       |
       | Help: use INT, or drop the sign
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: WORD := 65536;
       |                        ^^|^^
       |                          `---- the value does not fit in WORD
       |
       | Note: WORD holds 0 to 65535
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: UINT := 16#FFFFFFFF;
       |                        ^^^^^|^^^^^
       |                             `------- the value does not fit in UINT
       |
       | Note: UINT holds 0 to 65535
    ---'
    ");
}

#[rstest]
fn invalid_u32_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: UDINT := -1;
        test2: DWORD := 4294967296;
        test3: UDINT := 16#FFFFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: UDINT := -1;
       |                         ^|
       |                          `-- the value is negative and UDINT is unsigned
       |
       | Help: use DINT, or drop the sign
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: DWORD := 4294967296;
       |                         ^^^^^|^^^^
       |                              `------ the value does not fit in DWORD
       |
       | Note: DWORD holds 0 to 4294967295
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                         ^^^^^^|^^^^^^
       |                               `-------- the value does not fit in UDINT
       |
       | Note: UDINT holds 0 to 4294967295
    ---'
    ");
}

#[rstest]
fn invalid_u64_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: ULINT := -1;
        test2: LWORD := 18446744073709551616;
        test3: ULINT := 16#FFFFFFFFFFFFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: ULINT := -1;
       |                         ^|
       |                          `-- the value is negative and ULINT is unsigned
       |
       | Help: use LINT, or drop the sign
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: LWORD := 18446744073709551616;
       |                         ^^^^^^^^^^|^^^^^^^^^
       |                                   `----------- the value does not fit in LWORD
       |
       | Note: LWORD holds 0 to 18446744073709551615
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: ULINT := 16#FFFFFFFFFFFFFFFFFF;
       |                         ^^^^^^^^^^|^^^^^^^^^^
       |                                   `------------ the value does not fit in ULINT
       |
       | Note: ULINT holds 0 to 18446744073709551615
    ---'
    ");
}

#[rstest]
fn invalid_i8_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: SINT := -129;
        test2: SINT := 128;
        test3: SINT := 16#FFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: SINT := -129;
       |                        ^^|^
       |                          `--- the value does not fit in SINT
       |
       | Note: SINT holds -128 to 127
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: SINT := 128;
       |                        ^|^
       |                         `--- the value does not fit in SINT
       |
       | Note: SINT holds -128 to 127
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: SINT := 16#FFFF;
       |                        ^^^|^^^
       |                           `----- the value does not fit in SINT
       |
       | Note: a radix literal is a bit pattern too wide for the 8 bits of SINT
    ---'
    ");
}

#[rstest]
fn invalid_i16_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := -32769;
        test2: INT := 32768;
        test3: INT := 16#FFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -32769;
       |                       ^^^|^^
       |                          `---- the value does not fit in INT
       |
       | Note: INT holds -32768 to 32767
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 32768;
       |                       ^^|^^
       |                         `---- the value does not fit in INT
       |
       | Note: INT holds -32768 to 32767
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:23 ]
       |
     6 |         test3: INT := 16#FFFFFFFF;
       |                       ^^^^^|^^^^^
       |                            `------- the value does not fit in INT
       |
       | Note: a radix literal is a bit pattern too wide for the 16 bits of INT
    ---'
    ");
}

#[rstest]
fn invalid_i32_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := -2147483649;
        test2: INT := 2147483648;
        test3: UDINT := 16#FFFFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -2147483649;
       |                       ^^^^^|^^^^^
       |                            `------- the value does not fit in INT
       |
       | Note: INT holds -32768 to 32767
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 2147483648;
       |                       ^^^^^|^^^^
       |                            `------ the value does not fit in INT
       |
       | Note: INT holds -32768 to 32767
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                         ^^^^^^|^^^^^^
       |                               `-------- the value does not fit in UDINT
       |
       | Note: UDINT holds 0 to 4294967295
    ---'
    ");
}

#[rstest]
fn invalid_i64_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LINT := -9223372036854775809;
        test2: LINT := 9223372036854775807;
        test3: LINT := 16#FFFFFFFFFFFFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: LINT := -9223372036854775809;
       |                        ^^^^^^^^^^|^^^^^^^^^
       |                                  `----------- the value does not fit in LINT
       |
       | Note: LINT holds -9223372036854775808 to 9223372036854775807
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: LINT := 16#FFFFFFFFFFFFFFFFFF;
       |                        ^^^^^^^^^^|^^^^^^^^^^
       |                                  `------------ the value does not fit in LINT
       |
       | Note: a radix literal is a bit pattern too wide for the 64 bits of LINT
    ---'
    ");
}

// A typed literal is checked against its own type, as an untyped one is
// against its target. These passed, or stopped the build with an internal
// error.
#[rstest]
fn invalid_typed_integer_literals(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR s : SINT; b : BYTE; u : USINT; d : DINT; END_VAR
            s := SINT#300;
            b := BYTE#16#1FF;
            u := USINT#-1;
            d := DINT#3000000000;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             s := SINT#300;
       |                  ^^^^|^^^
       |                      `----- the value does not fit in SINT
       |
       | Note: SINT holds -128 to 127
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             b := BYTE#16#1FF;
       |                  ^^^^^|^^^^^
       |                       `------- the value does not fit in BYTE
       |
       | Note: BYTE holds 0 to 255
    ---'
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:6:18 ]
       |
     6 |             u := USINT#-1;
       |                  ^^^^|^^^
       |                      `----- the value is negative and USINT is unsigned
       |
       | Help: use SINT, or drop the sign
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:7:18 ]
       |
     7 |             d := DINT#3000000000;
       |                  ^^^^^^^|^^^^^^^
       |                         `--------- the value does not fit in DINT
       |
       | Note: DINT holds -2147483648 to 2147483647
    ---'
    ");
}

// A radix literal is a bit pattern, so nine bits do not fit a SINT, where
// eight do. A sign makes it a value again: -(16#FF) is -255.
#[rstest]
fn invalid_radix_literal_wider_than_its_signed_type(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR s : SINT; i : INT; END_VAR
            s := SINT#16#1FF;
            i := 16#1_0000;
            s := -(16#FF);
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             s := SINT#16#1FF;
       |                  ^^^^^|^^^^^
       |                       `------- the value does not fit in SINT
       |
       | Note: a radix literal is a bit pattern too wide for the 8 bits of SINT
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:18 ]
       |
     3 |         VAR s : SINT; i : INT; END_VAR
       |                       |
       |                       `-- 'i' is declared here
       |
     5 |             i := 16#1_0000;
       |                  ^^^^|^^^^
       |                      `------ the value does not fit in INT
       |
       | Note: a radix literal is a bit pattern too wide for the 16 bits of INT
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:18 ]
       |
     3 |         VAR s : SINT; i : INT; END_VAR
       |             |
       |             `-- 's' is declared here
       |
     6 |             s := -(16#FF);
       |                  ^^^^|^^^
       |                      `----- the value does not fit in SINT
       |
       | Note: SINT holds -128 to 127
    ---'
    ");
}

// A radix literal has at least one digit. `16#` and `16#1?2` were lexed
// whole and refused as numbers that did not parse.
#[rstest]
fn invalid_radix_literal_without_digits(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION F : WORD
            F := 16#;
            F := 16#1?2;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0001] Error: syntax error
       ,-[ file:///test0.st:3:20 ]
       |
     3 |             F := 16#;
       |                    |
       |                    `-- unexpected token(s): '#'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             F := 16#1?2;
       |                      ^|
       |                       `-- unexpected token(s): '?2'
    ---'
    ");
}

// A REAL or an LREAL too large for its type parsed to infinity.
#[rstest]
fn invalid_real_literal_past_its_range(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR r : REAL; l : LREAL; END_VAR
            r := 1.0E300;
            l := 1.0E400;
            r := REAL#1.0E39;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:4:18 ]
       |
     3 |         VAR r : REAL; l : LREAL; END_VAR
       |             |
       |             `-- 'r' is declared here
     4 |             r := 1.0E300;
       |                  ^^^|^^^
       |                     `----- the value does not fit in REAL
       |
       | Note: REAL holds magnitudes up to about 3.4E38
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:5:18 ]
       |
     3 |         VAR r : REAL; l : LREAL; END_VAR
       |                       |
       |                       `-- 'l' is declared here
       |
     5 |             l := 1.0E400;
       |                  ^^^|^^^
       |                     `----- the value does not fit in LREAL
       |
       | Note: LREAL holds magnitudes up to about 1.8E308
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:18 ]
       |
     6 |             r := REAL#1.0E39;
       |                  ^^^^^|^^^^^
       |                       `------- the value does not fit in REAL
       |
       | Note: REAL holds magnitudes up to about 3.4E38
    ---'
    ");
}

// A sign written apart from its literal is part of its value: `-(1)` and
// `- 1` are no UDINT, and `-(128)` is a SINT, where `128` alone is not.
// Parentheses alone leave the literal as written, a radix one a bit pattern.
#[rstest]
fn a_sign_apart_is_part_of_the_value(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
            u := -(1);
            v := - 1;
            s := -(129);
            ok := -(128);
            d := (16#FFFFFFFF);
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:4:18 ]
       |
     3 |         VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
       |             |
       |             `-- 'u' is declared here
     4 |             u := -(1);
       |                  ^^|^
       |                    `--- the value is negative and UDINT is unsigned
       |
       | Help: use DINT, or drop the sign
    ---'
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:5:18 ]
       |
     3 |         VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
       |                        |
       |                        `-- 'v' is declared here
       |
     5 |             v := - 1;
       |                  ^|^
       |                   `--- the value is negative and USINT is unsigned
       |
       | Help: use SINT, or drop the sign
    ---'
    [E0306] Error: literal out of range
       ,-[ file:///test0.st:6:18 ]
       |
     3 |         VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
       |                                   |
       |                                   `-- 's' is declared here
       |
     6 |             s := -(129);
       |                  ^^^|^^
       |                     `---- the value does not fit in SINT
       |
       | Note: SINT holds -128 to 127
    ---'
    ");
}
