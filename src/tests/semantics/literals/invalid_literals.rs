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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: BOOL := 256;
       |                       ^|^
       |                        `--- cannot infer '<integer>' to 'BOOL': invalid boolean literal
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: USINT := -1;
       |                         ^|
       |                          `-- cannot infer '<integer>' to 'USINT': literal can not be negative
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: BYTE := 256;
       |                        ^|^
       |                         `--- cannot infer '<integer>' to 'BYTE': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: USINT := 16#FFFF;
       |                         ^^^|^^^
       |                            `----- cannot infer '<integer>' to 'USINT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: UINT := -1;
       |                        ^|
       |                         `-- cannot infer '<integer>' to 'UINT': literal can not be negative
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: WORD := 65536;
       |                        ^^|^^
       |                          `---- cannot infer '<integer>' to 'WORD': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: UINT := 16#FFFFFFFF;
       |                        ^^^^^|^^^^^
       |                             `------- cannot infer '<integer>' to 'UINT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: UDINT := -1;
       |                         ^|
       |                          `-- cannot infer '<integer>' to 'UDINT': literal can not be negative
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: DWORD := 4294967296;
       |                         ^^^^^|^^^^
       |                              `------ cannot infer '<integer>' to 'DWORD': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                         ^^^^^^|^^^^^^
       |                               `-------- cannot infer '<integer>' to 'UDINT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: ULINT := -1;
       |                         ^|
       |                          `-- cannot infer '<integer>' to 'ULINT': literal can not be negative
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: LWORD := 18446744073709551616;
       |                         ^^^^^^^^^^|^^^^^^^^^
       |                                   `----------- cannot infer '<integer>' to 'LWORD': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: ULINT := 16#FFFFFFFFFFFFFFFFFF;
       |                         ^^^^^^^^^^|^^^^^^^^^^
       |                                   `------------ cannot infer '<integer>' to 'ULINT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: SINT := -129;
       |                        ^^|^
       |                          `--- cannot infer '<integer>' to 'SINT': number too small to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: SINT := 128;
       |                        ^|^
       |                         `--- cannot infer '<integer>' to 'SINT': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: SINT := 16#FFFF;
       |                        ^^^|^^^
       |                           `----- cannot infer '<integer>' to 'SINT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -32769;
       |                       ^^^|^^
       |                          `---- cannot infer '<integer>' to 'INT': number too small to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 32768;
       |                       ^^|^^
       |                         `---- cannot infer '<integer>' to 'INT': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:23 ]
       |
     6 |         test3: INT := 16#FFFFFFFF;
       |                       ^^^^^|^^^^^
       |                            `------- cannot infer '<integer>' to 'INT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -2147483649;
       |                       ^^^^^|^^^^^
       |                            `------- cannot infer '<integer>' to 'INT': number too small to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 2147483648;
       |                       ^^^^^|^^^^
       |                            `------ cannot infer '<integer>' to 'INT': number too large to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                         ^^^^^^|^^^^^^
       |                               `-------- cannot infer '<integer>' to 'UDINT': number too large to fit in target type
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: LINT := -9223372036854775809;
       |                        ^^^^^^^^^^|^^^^^^^^^
       |                                  `----------- cannot infer '<integer>' to 'LINT': number too small to fit in target type
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: LINT := 16#FFFFFFFFFFFFFFFFFF;
       |                        ^^^^^^^^^^|^^^^^^^^^^
       |                                  `------------ cannot infer '<integer>' to 'LINT': number too large to fit in target type
    ---'
    ");
}
