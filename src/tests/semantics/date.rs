use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// IEC 61131-3 Table 9 — Date literals

// 1a Date literal (long prefix)
#[rstest]
#[case("DATE#1984-06-25")]
#[case("date#2010-09-22")]
fn valid_date_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DATE := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 1b Date literal (short prefix)
#[rstest]
#[case("D#1984-06-25")]
fn valid_date_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DATE := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 2a Long date literal (long prefix)
#[rstest]
#[case("LDATE#2012-02-29")]
fn valid_ldate_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LDATE := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 2b Long date literal (short prefix)
#[rstest]
#[case("LD#1984-06-25")]
fn valid_ldate_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LDATE := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// DATE → LDATE implicit cast
#[rstest]
fn valid_date_to_ldate_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LDATE := DATE#1984-06-25;
        test2: LDATE := D#2010-09-22;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// LDATE → DATE narrowing is not allowed
#[rstest]
fn invalid_ldate_to_date(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DATE := LDATE#2012-02-29;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         test1: DATE := LDATE#2012-02-29;
       |                     ^^^^^^^^^|^^^^^^^^^  
       |                              `----------- expected 'DATE', got 'LDATE'
    ---'
    ");
}

// Type mismatch: assigning date literal to non-date type
#[rstest]
fn invalid_date_wrong_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := DATE#1984-06-25;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         test1: INT := DATE#1984-06-25;
       |                    ^^^^^^^^^|^^^^^^^^  
       |                             `---------- expected 'INT', got 'DATE'
    ---'
    ");
}
