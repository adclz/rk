use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// IEC 61131-3 Table 9 — Date and time literals

// 5a Date and time literal (long prefix)
#[rstest]
#[case("DATE_AND_TIME#1984-06-25-15:36:55.360227400")]
fn valid_dt_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DT := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 5b Date and time literal (short prefix)
#[rstest]
#[case("DT#1984-06-25-15:36:55.360_227_400")]
fn valid_dt_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DT := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 6a Long date and time literal (long prefix)
#[rstest]
#[case("LDATE_AND_TIME#1984-06-25-15:36:55.360_227_400")]
fn valid_ldt_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LDT := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// 6b Long date and time literal (short prefix)
#[rstest]
#[case("LDT#1984-06-25-15:36:55.360_227_400")]
fn valid_ldt_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LDT := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// DT → LDT implicit cast
#[rstest]
fn valid_dt_to_ldt_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LDT := DT#1984-06-25-15:36:55.360227400;
        test2: LDT := DATE_AND_TIME#1984-06-25-15:36:55.360227400;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// LDT → DT narrowing is not allowed
#[rstest]
fn invalid_ldt_to_dt(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: DT := LDT#1984-06-25-15:36:55.360227400;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:19 ]
       |
     4 |         test1: DT := LDT#1984-06-25-15:36:55.360227400;
       |                   ^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^
       |                                     `------------------- expected 'DT', got 'LDT'
    ---'
    ");
}

// Type mismatch: assigning DT literal to non-DT type
#[rstest]
fn invalid_dt_wrong_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := DT#1984-06-25-15:36:55.360227400;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         test1: INT := DT#1984-06-25-15:36:55.360227400;
       |                    ^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^
       |                                     `------------------- expected 'INT', got 'DT'
    ---'
    ");
}

// ── Integer encoding (DT → i64 secs, LDT → i64 ns since 1970-01-01) ──
//
// `DT` (i64 seconds) is bounded to LDT's span, ≈ 1677-09-21 to 2262-04-11,
// so the implicit DT → LDT widening can never overflow (the containment
// assertion in hir's literals.rs). Out-of-range literals on either type
// surface as E0306 with the supported bounds shown as IEC literals.

use crate::tests::semantics::literals::parse_literal;

#[rstest]
#[case("DT#1970-01-01-00:00:00", 0)]
#[case("DT#1970-01-01-00:00:01", 1)]
#[case("DT#1970-01-01-00:01:00", 60)]
#[case("DT#1970-01-02-00:00:00", 86_400)]
#[case("DT#2000-01-01-00:00:00", 946_684_800)]
#[case("DT#2100-01-01-00:00:00", 4_102_444_800)] // past 2038: representable only since i64
fn dt_secs_i64(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i64) {
    let id = parse_literal(&mut with_db, "DATE_AND_TIME", literal);
    assert_eq!(id.as_dt_secs_i64(&with_db).unwrap(), expected);
}

#[rstest]
fn dt_underflow_before_1677_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : DATE_AND_TIME := DT#1500-01-01-00:00:00;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:30 ]
       |
     4 |         x : DATE_AND_TIME := DT#1500-01-01-00:00:00;
       |                              ^^^^^^^^^^^|^^^^^^^^^^
       |                                         `------------ cannot infer 'DT literal' to 'DT': DT value is below the supported minimum
       |
       | Note: valid range for DT: DT#1677-09-21-00:12:44 to DT#2262-04-11-23:47:16
    ---'
    ");
}

#[rstest]
fn dt_overflow_after_2262_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : DATE_AND_TIME := DT#2500-01-01-00:00:00;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:30 ]
       |
     4 |         x : DATE_AND_TIME := DT#2500-01-01-00:00:00;
       |                              ^^^^^^^^^^^|^^^^^^^^^^
       |                                         `------------ cannot infer 'DT literal' to 'DT': DT value exceeds the supported maximum
       |
       | Note: valid range for DT: DT#1677-09-21-00:12:44 to DT#2262-04-11-23:47:16
    ---'
    ");
}

/// The old i32 encoding refused these; bounded-i64 accepts them. The 2038
/// cutoff is gone (the ceiling is LDT's 2262 now).
#[rstest]
#[case("DT#1800-01-01-00:00:00")]
#[case("DT#2200-01-01-00:00:00")]
fn dt_wide_range_is_valid(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        x : DATE_AND_TIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("LDT#1970-01-01-00:00:00", 0)]
#[case("LDT#1970-01-01-00:00:00.000000001", 1)]
#[case("LDT#1970-01-01-00:00:01", 1_000_000_000)]
#[case("LDT#1970-01-02-00:00:00", 86_400_000_000_000)]
fn ldt_ns_i64(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i64) {
    let id = parse_literal(&mut with_db, "LDATE_AND_TIME", literal);
    assert_eq!(id.as_ldt_ns_i64(&with_db).unwrap(), expected);
}

#[rstest]
fn ldt_overflow_after_2262_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : LDATE_AND_TIME := LDT#3000-01-01-00:00:00;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:31 ]
       |
     4 |         x : LDATE_AND_TIME := LDT#3000-01-01-00:00:00;
       |                               ^^^^^^^^^^^|^^^^^^^^^^^
       |                                          `------------- cannot infer 'LDT literal' to 'LDT': LDT value exceeds the supported maximum
       |
       | Note: valid range for LDT: LDT#1677-09-21-00:12:43.145224192 to LDT#2262-04-11-23:47:16.854775807
    ---'
    ");
}

#[rstest]
fn ldt_underflow_before_1677_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : LDATE_AND_TIME := LDT#1500-01-01-00:00:00;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:31 ]
       |
     4 |         x : LDATE_AND_TIME := LDT#1500-01-01-00:00:00;
       |                               ^^^^^^^^^^^|^^^^^^^^^^^
       |                                          `------------- cannot infer 'LDT literal' to 'LDT': LDT value is below the supported minimum
       |
       | Note: valid range for LDT: LDT#1677-09-21-00:12:43.145224192 to LDT#2262-04-11-23:47:16.854775807
    ---'
    ");
}

#[rstest]
fn dt_garbage_is_a_literal_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : DATE_AND_TIME := DT#garbage;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:30 ]
       |
     4 |         x : DATE_AND_TIME := DT#garbage;
       |                              ^^^^^|^^^^
       |                                   `------ cannot infer 'DT literal' to 'DT': expected the form DT#1984-06-25-15:36:55
    ---'
    ");
}
