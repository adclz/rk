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
       |                              |
       |                              `----------- consider explicitly casting with 'LDATE_TO_DATE(:= LDATE#2012-02-29)'
       |
       | Help: insert explicit cast 'LDATE_TO_DATE(:= LDATE#2012-02-29)'
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

// ── Integer encoding (DATE / LDATE → i32 / i64 days since 1970-01-01) ──
//
// Both encodings have effectively unlimited range (i32 days covers
// ±5.8M years), so no error tests are needed — value-correctness only.

use crate::tests::semantics::literals::parse_literal;

#[rstest]
#[case("D#1970-01-01", 0)]
#[case("D#1970-01-02", 1)]
#[case("D#1970-12-31", 364)]
#[case("D#1971-01-01", 365)]
// 1972 is a leap year (366 days). 1973-01-01 = 365+365+366 = 1096.
#[case("D#1973-01-01", 1_096)]
#[case("D#2020-01-01", 18_262)]
fn date_days_i32(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i32) {
    let id = parse_literal(&mut with_db, "DATE", literal);
    assert_eq!(id.as_date_days_i32(&with_db).unwrap(), expected);
}

#[rstest]
#[case("LD#1970-01-01", 0)]
#[case("LD#1970-01-02", 1)]
#[case("LD#2020-01-01", 18_262)]
fn ldate_days_i64(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i64) {
    let id = parse_literal(&mut with_db, "LDATE", literal);
    assert_eq!(id.as_ldate_days_i64(&with_db).unwrap(), expected);
}
