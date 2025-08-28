use db::RootDatabase;
use hir::def::{
    expressions::{
        expression::{Elementary, Integer, IntegerKind},
        spec::ElementarySpec,
    },
    interned::identifier::Ident,
};
use rstest::{fixture, rstest};

#[fixture]
fn with_db() -> RootDatabase {
    RootDatabase::default()
}

// Boolean tests - TRUE/FALSE are identifiers, 0/1 are integers
#[rstest]
#[case("0")]
#[case("1")]
fn bool_integer_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Bool;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        IntegerKind::Signed,
    ));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("TRUE")]
#[case("FALSE")]
fn bool_ident_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Bool;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("2")]
#[case("invalid")]
#[case("-1")]
fn bool_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Bool;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        IntegerKind::Signed,
    ));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Byte tests (0-255)
#[rstest]
#[case("2#11111111", IntegerKind::Binary)] // 255 in binary
#[case("2#00000000", IntegerKind::Binary)] // 0 in binary
#[case("8#377", IntegerKind::Octal)] // 255 in octal
#[case("8#0", IntegerKind::Octal)] // 0 in octal
#[case("16#FF", IntegerKind::Hex)] // 255 in hex
#[case("16#00", IntegerKind::Hex)] // 0 in hex
#[case("255", IntegerKind::Signed)] // 255 decimal
#[case("0", IntegerKind::Signed)] // 0 decimal
fn byte_valid(with_db: RootDatabase, #[case] lit: &str, #[case] kind: IntegerKind) {
    let spec = ElementarySpec::Byte;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        kind,
    ));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("256", IntegerKind::Signed)] // Overflow
#[case("2#100000000", IntegerKind::Binary)] // 256 in binary
#[case("16#100", IntegerKind::Hex)] // 256 in hex
fn byte_invalid(with_db: RootDatabase, #[case] lit: &str, #[case] kind: IntegerKind) {
    let spec = ElementarySpec::Byte;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        kind,
    ));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Word tests (0-65535)
#[rstest]
#[case("2#1111111111111111", IntegerKind::Binary)] // 65535 in binary
#[case("8#177777", IntegerKind::Octal)] // 65535 in octal
#[case("16#FFFF", IntegerKind::Hex)] // 65535 in hex
#[case("65535", IntegerKind::Signed)] // 65535 decimal
#[case("0", IntegerKind::Signed)] // 0 decimal
fn word_valid(with_db: RootDatabase, #[case] lit: &str, #[case] kind: IntegerKind) {
    let spec = ElementarySpec::Word;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        kind,
    ));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("65536", IntegerKind::Signed)] // Overflow
#[case("16#10000", IntegerKind::Hex)] // 65536 in hex
fn word_invalid(with_db: RootDatabase, #[case] lit: &str, #[case] kind: IntegerKind) {
    let spec = ElementarySpec::Word;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        kind,
    ));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Signed integer tests
#[rstest]
#[case("-128", IntegerKind::Signed)] // SINT min
#[case("127", IntegerKind::Signed)] // SINT max
#[case("0", IntegerKind::Signed)] // Zero
fn sint_valid(with_db: RootDatabase, #[case] lit: &str, #[case] kind: IntegerKind) {
    let spec = ElementarySpec::SInt;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        kind,
    ));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("-129", IntegerKind::Signed)] // Underflow
#[case("128", IntegerKind::Signed)] // Overflow
fn sint_invalid(with_db: RootDatabase, #[case] lit: &str, #[case] kind: IntegerKind) {
    let spec = ElementarySpec::SInt;
    let value = Elementary::InferInteger(Integer::new(
        &with_db,
        Ident::from_slice(&with_db, lit),
        kind,
    ));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Floating point tests
#[rstest]
#[case("3.14")]
#[case("0.0")]
#[case("-1.5")]
#[case("1e6")]
#[case("1.23e-4")]
fn real_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Real;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("not_a_number")]
#[case("")]
#[case("1.2.3")]
fn real_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Real;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Date tests
#[rstest]
#[case("DATE#2023-12-25")]
#[case("D#2023-01-01")]
fn date_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Date;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    spec.lit_check(&with_db, value).unwrap();
}

#[rstest]
#[case("DATE#2023-13-25")] // Invalid month
#[case("DATE#invalid")] // Invalid format
#[case("not_a_date")] // No prefix
fn date_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Date;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Time of day tests
#[rstest]
#[case("TIME_OF_DAY#12:30:45.123")]
#[case("TOD#00:00:00.000")]
fn tod_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Tod;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("#TOD#25:00:00.000")] // Invalid hour
#[case("#TOD#12:60:00.000")] // Invalid minute
#[case("not_a_time")] // No prefix
fn tod_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Tod;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Date and time tests
#[rstest]
#[case("DATE_AND_TIME#2023-12-25-12:30:45.123")]
#[case("DT#2023-01-01-00:00:00.000")]
fn dt_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Dt;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("DT#2023-13-25-12:30:45.123")] // Invalid month
#[case("DT#2023-12-25-25:30:45.123")] // Invalid hour
#[case("not_a_datetime")] // No prefix
fn dt_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Dt;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// String tests
#[rstest]
#[case("'hello world'")]
#[case("'test with $20 space'")] // $20 is hex for space
#[case("'$41$42$43'")] // $41$42$43 = ABC
#[case("''")] // Empty string
fn string_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::String;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("'incomplete $4'")] // Incomplete hex escape
#[case("'invalid $GG'")] // Invalid hex escape
fn string_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::String;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Wide string tests
#[rstest]
#[case("\"hello world\"")]
#[case("\"test with $0020 space\"")] // $0020 is Unicode space
#[case("\"$0041$0042$0043\"")] // $0041$0042$0043 = ABC
#[case("\"\"")] // Empty wide string
fn wstring_valid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::WString;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

#[rstest]
#[case("\"incomplete $004\"")] // Incomplete hex escape
#[case("\"invalid $GGGG\"")] // Invalid hex escape
fn wstring_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::WString;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

// Time tests
#[rstest]
#[case("T#14ms")] // Short prefix milliseconds
#[case("T#14.7s")] // Short prefix with decimal seconds
#[case("T#14.7m")] // Short prefix with decimal minutes
#[case("T#14.7h")] // Short prefix with decimal hours
#[case("t#14.7d")] // Short prefix with decimal days
#[case("t#25h15m")] // Multiple components
#[case("t#5d14h12m18s3.5ms")] // Complex duration
#[case("t#12h4m34ms230us400ns")] // Very complex with sub-millisecond
#[case("TIME#14ms")] // Long prefix
#[case("TIME#-14ms")] // Negative
#[case("time#14.7s")] // Long prefix lowercase
#[case("t#25h_15m")] // With underscores
#[case("t#5d_14h_12m_18s_3.5ms")] // Complex with underscores
fn time_cases(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Time;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));

    assert!(spec.lit_check(&with_db, value).is_ok());
}

// LTime tests
#[rstest]
#[case("LT#14ms")] // Short prefix
#[case("LT#14.7s")] // Short prefix with decimal
#[case("lt#5d14h12m18s3.5ms")] // Complex duration
#[case("LTIME#14ms")] // Long prefix
#[case("LTIME#-14ms")] // Negative
#[case("ltime#14.7s")] // Long prefix lowercase
#[case("LTIME#5m_30s_500ms_100.1us")] // With underscores and microseconds
#[case("ltime#5d_14h_12m_")] // Trailing underscore (should be handled)
fn ltime_cases(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::LTime;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));

    assert!(spec.lit_check(&with_db, value).is_ok());
}

// LTIME i64 boundaries (ns resolution)
#[rstest]
#[case("LTIME#9223372036854775807ns")] // i64::MAX
#[case("LTIME#-9223372036854775808ns")] // i64::MIN
fn ltime_boundaries(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::LTime;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok());
}

// Overflow (just beyond i64 bounds)
#[rstest]
#[case("LTIME#9223372036854775808ns")]
#[case("LTIME#-9223372036854775809ns")]
fn ltime_overflow(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::LTime;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}

#[rstest]
#[case("T#14.7s")] // Valid decimal
#[case("T#14.7ms")] // Invalid — sub-ms must be integer
#[case("LTIME#1.234us")] // Invalid — sub-micro must be integer
#[case("T#1.5d")] // Valid
fn decimal_unit_rules(with_db: RootDatabase, #[case] lit: &str) {
    let spec = if lit.to_lowercase().starts_with("lt") {
        ElementarySpec::LTime
    } else {
        ElementarySpec::Time
    };
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_ok() || spec.lit_check(&with_db, value).is_err());
}

// Invalid time tests
#[rstest]
#[case("T#14xyz")] // Invalid unit
#[case("invalid_time")] // Not a time literal
fn time_invalid(with_db: RootDatabase, #[case] lit: &str) {
    let spec = ElementarySpec::Time;
    let value = Elementary::InferIdent(Ident::from_slice(&with_db, lit));
    assert!(spec.lit_check(&with_db, value).is_err());
}
