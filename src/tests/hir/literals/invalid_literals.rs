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
    [E0308] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: BOOL := 256;
       |                       ^|^
       |                        `--- cannot infer '<integer>' to 'BOOL': invalid boolean literal; BOOL is TRUE or FALSE
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
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: USINT := -1;
       |                         ^|
       |                          `-- cannot infer '<integer>' to 'USINT': USINT cannot be negative; USINT is unsigned; use SINT, or drop the sign
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: BYTE := 256;
       |                        ^|^
       |                         `--- cannot infer '<integer>' to 'BYTE': the value does not fit in BYTE; BYTE holds 0 to 255
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: USINT := 16#FFFF;
       |                         ^^^|^^^
       |                            `----- cannot infer '<integer>' to 'USINT': the value does not fit in USINT; USINT holds 0 to 255
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
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: UINT := -1;
       |                        ^|
       |                         `-- cannot infer '<integer>' to 'UINT': UINT cannot be negative; UINT is unsigned; use INT, or drop the sign
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: WORD := 65536;
       |                        ^^|^^
       |                          `---- cannot infer '<integer>' to 'WORD': the value does not fit in WORD; WORD holds 0 to 65535
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: UINT := 16#FFFFFFFF;
       |                        ^^^^^|^^^^^
       |                             `------- cannot infer '<integer>' to 'UINT': the value does not fit in UINT; UINT holds 0 to 65535
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
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: UDINT := -1;
       |                         ^|
       |                          `-- cannot infer '<integer>' to 'UDINT': UDINT cannot be negative; UDINT is unsigned; use DINT, or drop the sign
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: DWORD := 4294967296;
       |                         ^^^^^|^^^^
       |                              `------ cannot infer '<integer>' to 'DWORD': the value does not fit in DWORD; DWORD holds 0 to 4294967295
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                         ^^^^^^|^^^^^^
       |                               `-------- cannot infer '<integer>' to 'UDINT': the value does not fit in UDINT; UDINT holds 0 to 4294967295
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
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: ULINT := -1;
       |                         ^|
       |                          `-- cannot infer '<integer>' to 'ULINT': ULINT cannot be negative; ULINT is unsigned; use LINT, or drop the sign
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: LWORD := 18446744073709551616;
       |                         ^^^^^^^^^^|^^^^^^^^^
       |                                   `----------- cannot infer '<integer>' to 'LWORD': the value does not fit in LWORD; LWORD holds 0 to 18446744073709551615
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: ULINT := 16#FFFFFFFFFFFFFFFFFF;
       |                         ^^^^^^^^^^|^^^^^^^^^^
       |                                   `------------ cannot infer '<integer>' to 'ULINT': the value does not fit in ULINT; ULINT holds 0 to 18446744073709551615
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: SINT := -129;
       |                        ^^|^
       |                          `--- cannot infer '<integer>' to 'SINT': the value does not fit in SINT; SINT holds -128 to 127
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: SINT := 128;
       |                        ^|^
       |                         `--- cannot infer '<integer>' to 'SINT': the value does not fit in SINT; SINT holds -128 to 127
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: SINT := 16#FFFF;
       |                        ^^^|^^^
       |                           `----- cannot infer '<integer>' to 'SINT': the value does not fit in SINT; SINT holds -128 to 127
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -32769;
       |                       ^^^|^^
       |                          `---- cannot infer '<integer>' to 'INT': the value does not fit in INT; INT holds -32768 to 32767
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 32768;
       |                       ^^|^^
       |                         `---- cannot infer '<integer>' to 'INT': the value does not fit in INT; INT holds -32768 to 32767
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:23 ]
       |
     6 |         test3: INT := 16#FFFFFFFF;
       |                       ^^^^^|^^^^^
       |                            `------- cannot infer '<integer>' to 'INT': the value does not fit in INT; INT holds -32768 to 32767
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -2147483649;
       |                       ^^^^^|^^^^^
       |                            `------- cannot infer '<integer>' to 'INT': the value does not fit in INT; INT holds -32768 to 32767
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 2147483648;
       |                       ^^^^^|^^^^
       |                            `------ cannot infer '<integer>' to 'INT': the value does not fit in INT; INT holds -32768 to 32767
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                         ^^^^^^|^^^^^^
       |                               `-------- cannot infer '<integer>' to 'UDINT': the value does not fit in UDINT; UDINT holds 0 to 4294967295
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: LINT := -9223372036854775809;
       |                        ^^^^^^^^^^|^^^^^^^^^
       |                                  `----------- cannot infer '<integer>' to 'LINT': the value does not fit in LINT; LINT holds -9223372036854775808 to 9223372036854775807
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: LINT := 16#FFFFFFFFFFFFFFFFFF;
       |                        ^^^^^^^^^^|^^^^^^^^^^
       |                                  `------------ cannot infer '<integer>' to 'LINT': the value does not fit in LINT; LINT holds -9223372036854775808 to 9223372036854775807
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             s := SINT#300;
       |                  ^^^^|^^^
       |                      `----- cannot infer 'SINT literal' to 'SINT': the value does not fit in SINT; SINT holds -128 to 127
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             b := BYTE#16#1FF;
       |                  ^^^^^|^^^^^
       |                       `------- cannot infer 'BYTE literal' to 'BYTE': the value does not fit in BYTE; BYTE holds 0 to 255
    ---'
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:6:18 ]
       |
     6 |             u := USINT#-1;
       |                  ^^^^|^^^
       |                      `----- cannot infer 'USINT literal' to 'USINT': USINT cannot be negative; USINT is unsigned; use SINT, or drop the sign
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:7:18 ]
       |
     7 |             d := DINT#3000000000;
       |                  ^^^^^^^|^^^^^^^
       |                         `--------- cannot infer 'DINT literal' to 'DINT': the value does not fit in DINT; DINT holds -2147483648 to 2147483647
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
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:18 ]
       |
     3 |         VAR r : REAL; l : LREAL; END_VAR
       |             |
       |             `-- type is declared by variable 'r' here
     4 |             r := 1.0E300;
       |                  ^^^|^^^
       |                     `----- cannot infer '<float>' to 'REAL': the value does not fit in REAL; REAL holds magnitudes up to about 3.4E38
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:5:18 ]
       |
     3 |         VAR r : REAL; l : LREAL; END_VAR
       |                       |
       |                       `-- type is declared by variable 'l' here
       |
     5 |             l := 1.0E400;
       |                  ^^^|^^^
       |                     `----- cannot infer '<float>' to 'LREAL': the value does not fit in LREAL; LREAL holds magnitudes up to about 1.8E308
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:18 ]
       |
     6 |             r := REAL#1.0E39;
       |                  ^^^^^|^^^^^
       |                       `------- cannot infer 'REAL literal' to 'REAL': the value does not fit in REAL; REAL holds magnitudes up to about 3.4E38
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
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:4:18 ]
       |
     3 |         VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
       |             |
       |             `-- type is declared by variable 'u' here
     4 |             u := -(1);
       |                  ^^|^
       |                    `--- cannot infer '<unary expression>' to 'UDINT': UDINT cannot be negative; UDINT is unsigned; use DINT, or drop the sign
    ---'
    [E0307] Error: invalid literal
       ,-[ file:///test0.st:5:18 ]
       |
     3 |         VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
       |                        |
       |                        `-- type is declared by variable 'v' here
       |
     5 |             v := - 1;
       |                  ^|^
       |                   `--- cannot infer '<unary expression>' to 'USINT': USINT cannot be negative; USINT is unsigned; use SINT, or drop the sign
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:6:18 ]
       |
     3 |         VAR u : UDINT; v : USINT; s : SINT; ok : SINT; d : DINT; END_VAR
       |                                   |
       |                                   `-- type is declared by variable 's' here
       |
     6 |             s := -(129);
       |                  ^^^|^^
       |                     `---- cannot infer '<unary expression>' to 'SINT': the value does not fit in SINT; SINT holds -128 to 127
    ---'
    ");
}
