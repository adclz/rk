use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// IEC 61131-3 Table 2a - TIME literals, short prefix, no underscore
// TIME is 32-bit, ms resolution: d, h, m, s, ms are valid; us and ns are not
#[rstest]
#[case("T#14ms")]
#[case("T#-14ms")]
#[case("T#14.7h")]
#[case("t#14.7d")]
#[case("t#25h15m")]
#[case("t#12h4m34ms230us400ns")]
fn valid_time_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// IEC 61131-3 Table 2b - TIME literals, long prefix, no underscore
#[rstest]
#[case("TIME#14ms")]
#[case("TIME#-14ms")]
#[case("time#14.7s")]
fn valid_time_long_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// IEC 61131-3 Table 3a - TIME literals, short prefix, with underscore
#[rstest]
#[case("t#25h_15m")]
#[case("t#5d_14h_12m_18s_3.5ms")]
fn valid_time_short_prefix_underscore(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// IEC 61131-3 Table 3b - TIME literals, long prefix, with underscore
#[rstest]
#[case("TIME#25h_15m")]
fn valid_time_long_prefix_underscore(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// IEC 61131-3 Table 2a - LTIME literals, short prefix, no underscore
// LTIME is 64-bit, ns resolution: all units (d, h, m, s, ms, us, ns) are valid
#[rstest]
#[case("LT#14.7s")]
#[case("LT#14.7m")]
#[case("lt#5d14h12m18s3.5ms")]
#[case("lt#12h4m34ms230us400ns")]
fn valid_ltime_short_prefix(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// IEC 61131-3 Table 3a - LTIME literals, short prefix/long prefix, with underscore
#[rstest]
#[case("LTIME#5m_30s_500ms_100.1us")]
fn valid_ltime_us_ns(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// IEC 61131-3 Table 3b - LTIME literals, long prefix, with underscore
#[rstest]
#[case("ltime#5d_14h_12m_18s_3.5ms")]
#[case("LTIME#34s_345ns")]
fn valid_ltime_long_prefix_underscore(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTIME := {value};
    END_VAR
END_FUNCTION_BLOCK"#
    );
    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// Negative durations
#[rstest]
fn valid_negative_duration(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := T#-5s;
        test2: TIME := TIME#-14ms;
        test3: LTIME := LT#-1h30m;
        test4: LTIME := LTIME#-100us;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// TIME → LTIME implicit cast (TIME widens to LTIME per IEC 61131-3)
#[rstest]
fn valid_time_to_ltime_implicit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: LTIME := T#5s;
        test2: LTIME := TIME#1h30m;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// Type mismatch: assigning a duration literal to a non-duration type
#[rstest]
fn invalid_duration_wrong_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: INT := T#5s;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:20 ]
       |
     4 |         test1: INT := T#5s;
       |                    ^^^|^^^
       |                       `----- expected 'INT', got 'TIME'
    ---'
    ");
}

// Type mismatch: LTIME cannot be assigned to TIME (no implicit narrowing)
#[rstest]
fn invalid_ltime_to_time(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: TIME := LTIME#5s;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         test1: TIME := LTIME#5s;
       |                     ^^^^^|^^^^^
       |                          `------- expected 'TIME', got 'LTIME'
       |                          |
       |                          `------- consider explicitly casting with 'LTIME_TO_TIME(:= LTIME#5s)'
       |
       | Help: insert explicit cast 'LTIME_TO_TIME(:= LTIME#5s)'
    ---'
    ");
}
