use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
#[case("SINT")]
fn valid_i8_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := -128;
        test2: {typ} := 127;
        test3: {typ} := SINT#120;
        test7: {typ} := BYTE#10;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("INT")]
fn valid_i16_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := -32768;
        test2: {typ} := 32767;
        test3: {typ} := SINT#120;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
        test7: {typ} := BYTE#10
        test8: {typ} := WORD#10
        test9: {typ} := INT#10
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("DINT")]
fn valid_i32_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := -2147483648;
        test2: {typ} := 2147483647;
        test3: {typ} := BYTE#120;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
        test7: {typ} := DWORD#10
        test8: {typ} := DINT#10
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("LINT")]
fn valid_i64_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := -9223372036854775808;
        test2: {typ} := 9223372036854775807;
        test3: {typ} := BYTE#120;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
        test7: {typ} := LWORD#10
        test8: {typ} := LINT#10
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
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
    Error:
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: SINT := -129;
       |                ^^|^    ^^|^
       |                  `----------- expected type 'SINT' here
       |                          |
       |                          `--- invalid value initializer: number too small to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: SINT := 128;
       |                ^^|^    ^|^
       |                  `---------- expected type 'SINT' here
       |                         |
       |                         `--- invalid value initializer: number too large to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: SINT := 16#FFFF;
       |                ^^|^    ^^^|^^^
       |                  `-------------- expected type 'SINT' here
       |                           |
       |                           `----- invalid value initializer: number too large to fit in target type
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
    Error:
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -32769;
       |                ^|^    ^^^|^^
       |                 `------------- expected type 'INT' here
       |                          |
       |                          `---- invalid value initializer: number too small to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 32768;
       |                ^|^    ^^|^^
       |                 `------------ expected type 'INT' here
       |                         |
       |                         `---- invalid value initializer: number too large to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:6:23 ]
       |
     6 |         test3: INT := 16#FFFFFFFF;
       |                ^|^    ^^^^^|^^^^^
       |                 `------------------ expected type 'INT' here
       |                            |
       |                            `------- invalid value initializer: number too large to fit in target type
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
    Error:
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test1: INT := -2147483649;
       |                ^|^    ^^^^^|^^^^^
       |                 `------------------ expected type 'INT' here
       |                            |
       |                            `------- invalid value initializer: number too small to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:5:23 ]
       |
     5 |         test2: INT := 2147483648;
       |                ^|^    ^^^^^|^^^^
       |                 `----------------- expected type 'INT' here
       |                            |
       |                            `------ invalid value initializer: number too large to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |                ^^|^^    ^^^^^^|^^^^^^
       |                  `--------------------- expected type 'UDINT' here
       |                               |
       |                               `-------- invalid value initializer: number too large to fit in target type
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
    Error:
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: LINT := -9223372036854775809;
       |                ^^|^    ^^^^^^^^^^|^^^^^^^^^
       |                  `--------------------------- expected type 'LINT' here
       |                                  |
       |                                  `----------- invalid value initializer: number too small to fit in target type
    ---'
    Error:
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: LINT := 16#FFFFFFFFFFFFFFFFFF;
       |                ^^|^    ^^^^^^^^^^|^^^^^^^^^^
       |                  `---------------------------- expected type 'LINT' here
       |                                  |
       |                                  `------------ invalid value initializer: number too large to fit in target type
    ---'
    ");
}
