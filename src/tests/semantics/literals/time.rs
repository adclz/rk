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
    ---'
    ");
}

// ── Integer encoding (TIME → i32 ms, LTIME → i64 ns) ────────────────────
//
// TIME literals decode to `i32` milliseconds (range ≈ ±24.8 days);
// LTIME to `i64` nanoseconds (≈ ±292 years). Out-of-range literals
// surface as E0306 with the supported bounds shown as IEC literals.

use crate::tests::semantics::literals::parse_literal;

#[rstest]
#[case("T#0ms", 0)]
#[case("T#1ms", 1)]
#[case("T#1s", 1_000)]
#[case("T#1m", 60_000)]
#[case("T#1h", 3_600_000)]
#[case("T#1d", 86_400_000)]
#[case("T#-1d", -86_400_000)]
fn time_ms_i32(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i32) {
    let id = parse_literal(&mut with_db, "TIME", literal);
    assert_eq!(id.as_time_ms_i32(&with_db).unwrap(), expected);
}

#[rstest]
fn time_overflow_more_than_max_i32_ms_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : TIME := T#100d;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         x : TIME := T#100d;
       |                     ^^^|^^
       |                        `---- cannot infer 'TIME literal' to 'TIME': TIME value exceeds the supported maximum; TIME holds T#-24d20h31m23s648ms to T#24d20h31m23s647ms
    ---'
    ");
}

#[rstest]
fn time_underflow_less_than_min_i32_ms_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : TIME := T#-100d;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         x : TIME := T#-100d;
       |                     ^^^|^^^
       |                        `----- cannot infer 'TIME literal' to 'TIME': TIME value is below the supported minimum; TIME holds T#-24d20h31m23s648ms to T#24d20h31m23s647ms
    ---'
    ");
}

#[rstest]
#[case("LT#0ns", 0)]
#[case("LT#1ns", 1)]
#[case("LT#1us", 1_000)]
#[case("LT#1ms", 1_000_000)]
#[case("LT#1s", 1_000_000_000)]
#[case("LT#-1s", -1_000_000_000)]
fn ltime_ns_i64(mut with_db: RootDatabase, #[case] literal: &str, #[case] expected: i64) {
    let id = parse_literal(&mut with_db, "LTIME", literal);
    assert_eq!(id.as_ltime_ns_i64(&with_db).unwrap(), expected);
}

#[rstest]
fn ltime_overflow_diagnostic(mut with_db: RootDatabase) {
    // `LT#9999999d` exceeds i64 ns (max ≈ 106751 days).
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : LTIME := LT#9999999d;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:22 ]
       |
     4 |         x : LTIME := LT#9999999d;
       |                      ^^^^^|^^^^^
       |                           `------- cannot infer 'LTIME literal' to 'LTIME': LTIME value exceeds the supported maximum; LTIME holds LT#-106751d23h47m16s854ms775us808ns to LT#106751d23h47m16s854ms775us807ns
    ---'
    ");
}

#[rstest]
fn ltime_underflow_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : LTIME := LT#-9999999d;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:4:22 ]
       |
     4 |         x : LTIME := LT#-9999999d;
       |                      ^^^^^^|^^^^^
       |                            `------- cannot infer 'LTIME literal' to 'LTIME': LTIME value is below the supported minimum; LTIME holds LT#-106751d23h47m16s854ms775us808ns to LT#106751d23h47m16s854ms775us807ns
    ---'
    ");
}

// A bad unit used to shred into an identifier at the CST (`y` reported as
// E0201 "no item found in scope" plus a syntax error). The value now lexes
// liberally and the HIR names the unit.
#[rstest]
fn invalid_duration_unit_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : TIME := T#2y;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         x : TIME := T#2y;
       |                     ^^|^
       |                       `--- cannot infer 'TIME literal' to 'TIME': 'y' is not a valid duration unit: use d, h, m, s, ms, us or ns; TIME is written T#1d2h3m4s5ms
    ---'
    ");
}

#[rstest]
fn invalid_ltime_unit_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : LTIME := LT#292y;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:22 ]
       |
     4 |         x : LTIME := LT#292y;
       |                      ^^^|^^^
       |                         `----- cannot infer 'LTIME literal' to 'LTIME': 'y' is not a valid duration unit: use d, h, m, s, ms, us or ns; LTIME is written LT#1d2h3m4s5ms
    ---'
    ");
}

// A number with no unit at all used to be a syntax error; it is a literal
// diagnostic now, symmetric with the bad-unit case.
#[rstest]
fn duration_missing_unit_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : TIME := T#5;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         x : TIME := T#5;
       |                     ^|^
       |                      `--- cannot infer 'TIME literal' to 'TIME': a TIME component is missing its unit: use d, h, m, s, ms, us or ns; TIME is written T#1d2h3m4s5ms
    ---'
    ");
}

#[rstest]
fn duration_trailing_component_missing_unit_diagnostic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : TIME := T#1h30;
    END_VAR
END_FUNCTION_BLOCK"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:4:21 ]
       |
     4 |         x : TIME := T#1h30;
       |                     ^^^|^^
       |                        `---- cannot infer 'TIME literal' to 'TIME': a TIME component is missing its unit: use d, h, m, s, ms, us or ns; TIME is written T#1d2h3m4s5ms
    ---'
    ");
}
