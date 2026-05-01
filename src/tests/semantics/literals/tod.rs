use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// IEC 61131-3 Table 9 — Time of day literals

// 3a Time of day literal (long prefix)
#[rstest]
#[case("TIME_OF_DAY#15:36:55.36")]
fn valid_tod_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TOD := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 3b Time of day literal (short prefix)
#[rstest]
#[case("TOD#15:36:55.36")]
#[case("tod#00:00:00.0")]
#[case("TOD#12:20:50.552")]
fn valid_tod_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TOD := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 4a Long time of day literal (short prefix)
#[rstest]
#[case("LTOD#15:36:55.36")]
#[case("ltod#00:00:00.0")]
#[case("LTOD#12:20:50.552")]
fn valid_ltod_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTOD := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 4b Long time of day literal (long prefix)
#[rstest]
#[case("LTIME_OF_DAY#15:36:55.36")]
fn valid_ltod_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTOD := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// TOD > LTOD implicit cast
#[rstest]
fn valid_tod_to_ltod_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTOD := TOD#15:36:55.36;
        test2: LTOD := TIME_OF_DAY#12:30:45.123;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// LTOD > TOD narrowing is not allowed
#[rstest]
fn invalid_ltod_to_tod(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TOD := LTOD#15:36:55.36;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         test1: TOD := LTOD#15:36:55.36;
       |                    ^^^^^^^^^|^^^^^^^^^
       |                             `----------- expected 'TOD', got 'LTOD'
       |                             |
       |                             `----------- consider explicitly casting with 'LTOD_TO_TOD(:= LTOD#15:36:55.36)'
       |
       | Help: insert explicit cast 'LTOD_TO_TOD(:= LTOD#15:36:55.36)'
    ---'
    ");
}

// Type mismatch: assigning TOD literal to non-TOD type
#[rstest]
fn invalid_tod_wrong_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := TOD#15:36:55.36;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         test1: INT := TOD#15:36:55.36;
       |                    ^^^^^^^^^|^^^^^^^^
       |                             `---------- expected 'INT', got 'TOD'
    ---'
    ");
}

// -- Integer encoding (TOD > i32 ms, LTOD > i64 ns since midnight) -----
//
// 24h fits in both `i32` ms (max 86_400_000) and `i64` ns
// (max 86.4×10¹²), so no error tests are needed.

use crate::tests::semantics::literals::parse_literal;

#[rstest]
#[case("TOD#00:00:00", 0)]
#[case("TOD#00:00:01", 1_000)]
#[case("TOD#00:01:00", 60_000)]
#[case("TOD#01:00:00", 3_600_000)]
#[case("TOD#23:59:59", 86_399_000)]
#[case("TOD#12:34:56.789", 12 * 3_600_000 + 34 * 60_000 + 56_000 + 789)]
fn tod_ms_i32(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i32) {
    let id = parse_literal(&mut with_db, "TOD", literal);
    assert_eq!(id.as_tod_ms_i32(&with_db).unwrap(), expected);
}

#[rstest]
#[case("LTOD#00:00:00", 0)]
#[case("LTOD#00:00:00.000000001", 1)]
#[case("LTOD#00:00:01", 1_000_000_000)]
#[case("LTOD#23:59:59.999999999", 86_399_999_999_999)]
fn ltod_ns_i64(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i64) {
    let id = parse_literal(&mut with_db, "LTOD", literal);
    assert_eq!(id.as_ltod_ns_i64(&with_db).unwrap(), expected);
}
