use db::RootDatabase;
use hir::def::{
    expressions::expression::{Numeric, NumericKind},
    interned::identifier::Ident,
};

#[test]
fn byte() {
    let db = RootDatabase::default();

    let b2 = Numeric::new(&db, Ident::from_slice(&db, "2#001"), NumericKind::Binary);
    let b8 = Numeric::new(&db, Ident::from_slice(&db, "8#11"), NumericKind::Octal);
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#FF"), NumericKind::Hex);
    let b10 = Numeric::new(&db, Ident::from_slice(&db, "255"), NumericKind::Signed);

    assert!(b2.as_u8(&db).is_ok());
    assert!(b8.as_u8(&db).is_ok());
    assert!(b16.as_u8(&db).is_ok());
    assert!(b10.as_u8(&db).is_ok());
}

#[test]
fn byte_invalid() {
    let db = RootDatabase::default();

    // Values too large for u8 (max 255)
    let b2_large = Numeric::new(
        &db,
        Ident::from_slice(&db, "2#100000000"),
        NumericKind::Binary,
    ); // 256 in binary
    let b8_large = Numeric::new(&db, Ident::from_slice(&db, "8#400"), NumericKind::Octal); // 256 in octal
    let b16_large = Numeric::new(&db, Ident::from_slice(&db, "16#100"), NumericKind::Hex); // 256 in hex
    let b10_large = Numeric::new(&db, Ident::from_slice(&db, "256"), NumericKind::Signed); // 256 in decimal

    assert!(b2_large.as_u8(&db).is_err());
    assert!(b8_large.as_u8(&db).is_err());
    assert!(b16_large.as_u8(&db).is_err());
    assert!(b10_large.as_u8(&db).is_err());
}

#[test]
fn sint() {
    let db = RootDatabase::default();

    let b2 = Numeric::new(&db, Ident::from_slice(&db, "2#001"), NumericKind::Binary);
    let b8 = Numeric::new(&db, Ident::from_slice(&db, "8#11"), NumericKind::Octal);
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#FF"), NumericKind::Hex);
    let b10 = Numeric::new(&db, Ident::from_slice(&db, "255"), NumericKind::Signed);

    assert!(b2.as_i32(&db).is_ok());
    assert!(b8.as_i32(&db).is_ok());
    assert!(b16.as_i32(&db).is_ok());
    assert!(b10.as_i32(&db).is_ok());
}

#[test]
fn usint() {
    let db = RootDatabase::default();

    // USINT: 0 to 255 (same as BYTE but semantically different)
    let b2 = Numeric::new(
        &db,
        Ident::from_slice(&db, "2#11111111"),
        NumericKind::Binary,
    ); // 255
    let b8 = Numeric::new(&db, Ident::from_slice(&db, "8#377"), NumericKind::Octal); // 255
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#FF"), NumericKind::Hex); // 255
    let b10 = Numeric::new(&db, Ident::from_slice(&db, "255"), NumericKind::Signed);

    assert!(b2.as_u8(&db).is_ok());
    assert!(b8.as_u8(&db).is_ok());
    assert!(b16.as_u8(&db).is_ok());
    assert!(b10.as_u8(&db).is_ok());
}

#[test]
fn uint() {
    let db = RootDatabase::default();

    // UINT: 0 to 65535
    let b2 = Numeric::new(
        &db,
        Ident::from_slice(&db, "2#1111111111111111"),
        NumericKind::Binary,
    ); // 65535
    let b8 = Numeric::new(&db, Ident::from_slice(&db, "8#177777"), NumericKind::Octal); // 65535
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#FFFF"), NumericKind::Hex); // 65535
    let b10 = Numeric::new(&db, Ident::from_slice(&db, "65535"), NumericKind::Signed);

    assert!(b2.as_u16(&db).is_ok());
    assert!(b8.as_u16(&db).is_ok());
    assert!(b16.as_u16(&db).is_ok());
    assert!(b10.as_u16(&db).is_ok());
}

#[test]
fn uint_invalid() {
    let db = RootDatabase::default();

    // Values too large for u16 (max 65535)
    let b16_large = Numeric::new(&db, Ident::from_slice(&db, "16#10000"), NumericKind::Hex); // 65536
    let b10_large = Numeric::new(&db, Ident::from_slice(&db, "65536"), NumericKind::Signed); // 65536

    assert!(b16_large.as_u16(&db).is_err());
    assert!(b10_large.as_u16(&db).is_err());
}

#[test]
fn udint() {
    let db = RootDatabase::default();

    // UDINT: 0 to 4294967295
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#FFFFFFFF"), NumericKind::Hex); // 4294967295
    let b10 = Numeric::new(
        &db,
        Ident::from_slice(&db, "4294967295"),
        NumericKind::Signed,
    ); // 4294967295

    assert!(b16.as_u32(&db).is_ok());
    assert!(b10.as_u32(&db).is_ok());
}

#[test]
fn dint() {
    let db = RootDatabase::default();

    // DINT: -2147483648 to 2147483647
    let b16_max = Numeric::new(&db, Ident::from_slice(&db, "16#7FFFFFFF"), NumericKind::Hex); // 2147483647
    let b10_max = Numeric::new(
        &db,
        Ident::from_slice(&db, "2147483647"),
        NumericKind::Signed,
    );
    let b10_min = Numeric::new(
        &db,
        Ident::from_slice(&db, "-2147483648"),
        NumericKind::Signed,
    );

    assert!(b16_max.as_i32(&db).is_ok());
    assert!(b10_max.as_i32(&db).is_ok());
    assert!(b10_min.as_i32(&db).is_ok());
}

#[test]
fn ulint() {
    let db = RootDatabase::default();

    // ULINT: 0 to 18446744073709551615
    let b16 = Numeric::new(
        &db,
        Ident::from_slice(&db, "16#FFFFFFFFFFFFFFFF"),
        NumericKind::Hex,
    );
    let b10 = Numeric::new(
        &db,
        Ident::from_slice(&db, "18446744073709551615"),
        NumericKind::Signed,
    );

    assert!(b16.as_u64(&db).is_ok());
    assert!(b10.as_u64(&db).is_ok());
}

#[test]
fn lint() {
    let db = RootDatabase::default();

    // LINT: -9223372036854775808 to 9223372036854775807
    let b16_max = Numeric::new(
        &db,
        Ident::from_slice(&db, "16#7FFFFFFFFFFFFFFF"),
        NumericKind::Hex,
    );
    let b10_max = Numeric::new(
        &db,
        Ident::from_slice(&db, "9223372036854775807"),
        NumericKind::Signed,
    );
    let b10_min = Numeric::new(
        &db,
        Ident::from_slice(&db, "-9223372036854775808"),
        NumericKind::Signed,
    );

    assert!(b16_max.as_i64(&db).is_ok());
    assert!(b10_max.as_i64(&db).is_ok());
    assert!(b10_min.as_i64(&db).is_ok());
}

#[test]
fn word() {
    let db = RootDatabase::default();

    // WORD: 16-bit unsigned (same range as UINT)
    let b2 = Numeric::new(
        &db,
        Ident::from_slice(&db, "2#1010101010101010"),
        NumericKind::Binary,
    );
    let b8 = Numeric::new(&db, Ident::from_slice(&db, "8#125252"), NumericKind::Octal);
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#AAAA"), NumericKind::Hex);
    let b10 = Numeric::new(&db, Ident::from_slice(&db, "43690"), NumericKind::Signed);

    assert!(b2.as_u16(&db).is_ok());
    assert!(b8.as_u16(&db).is_ok());
    assert!(b16.as_u16(&db).is_ok());
    assert!(b10.as_u16(&db).is_ok());
}

#[test]
fn dword() {
    let db = RootDatabase::default();

    // DWORD: 32-bit unsigned (same range as UDINT)
    let b2 = Numeric::new(
        &db,
        Ident::from_slice(&db, "2#10101010101010101010101010101010"),
        NumericKind::Binary,
    );
    let b16 = Numeric::new(&db, Ident::from_slice(&db, "16#AAAAAAAA"), NumericKind::Hex);
    let b10 = Numeric::new(
        &db,
        Ident::from_slice(&db, "2863311530"),
        NumericKind::Signed,
    );

    assert!(b2.as_u32(&db).is_ok());
    assert!(b16.as_u32(&db).is_ok());
    assert!(b10.as_u32(&db).is_ok());
}

#[test]
fn lword() {
    let db = RootDatabase::default();

    // LWORD: 64-bit unsigned (same range as ULINT)
    let b16 = Numeric::new(
        &db,
        Ident::from_slice(&db, "16#AAAAAAAAAAAAAAAA"),
        NumericKind::Hex,
    );
    let b10 = Numeric::new(
        &db,
        Ident::from_slice(&db, "12297829382473034410"),
        NumericKind::Signed,
    );

    assert!(b16.as_u64(&db).is_ok());
    assert!(b10.as_u64(&db).is_ok());
}

#[test]
fn edge_cases_binary() {
    let db = RootDatabase::default();

    // Test binary edge cases
    let zero = Numeric::new(&db, Ident::from_slice(&db, "2#0"), NumericKind::Binary);
    let one = Numeric::new(&db, Ident::from_slice(&db, "2#1"), NumericKind::Binary);
    let leading_zeros = Numeric::new(&db, Ident::from_slice(&db, "2#000001"), NumericKind::Binary);

    assert!(zero.as_u8(&db).is_ok());
    assert!(one.as_u8(&db).is_ok());
    assert!(leading_zeros.as_u8(&db).is_ok());
    assert_eq!(leading_zeros.as_u8(&db).unwrap(), 1);
}

#[test]
fn edge_cases_octal() {
    let db = RootDatabase::default();

    // Test octal edge cases
    let zero = Numeric::new(&db, Ident::from_slice(&db, "8#0"), NumericKind::Octal);
    let seven = Numeric::new(&db, Ident::from_slice(&db, "8#7"), NumericKind::Octal);
    let leading_zeros = Numeric::new(&db, Ident::from_slice(&db, "8#000007"), NumericKind::Octal);

    assert!(zero.as_u8(&db).is_ok());
    assert!(seven.as_u8(&db).is_ok());
    assert!(leading_zeros.as_u8(&db).is_ok());
    assert_eq!(leading_zeros.as_u8(&db).unwrap(), 7);
}

#[test]
fn edge_cases_hex() {
    let db = RootDatabase::default();

    // Test hex edge cases
    let zero = Numeric::new(&db, Ident::from_slice(&db, "16#0"), NumericKind::Hex);
    let f = Numeric::new(&db, Ident::from_slice(&db, "16#F"), NumericKind::Hex);
    let lowercase = Numeric::new(&db, Ident::from_slice(&db, "16#ff"), NumericKind::Hex);
    let leading_zeros = Numeric::new(&db, Ident::from_slice(&db, "16#000F"), NumericKind::Hex);

    assert!(zero.as_u8(&db).is_ok());
    assert!(f.as_u8(&db).is_ok());
    assert!(lowercase.as_u8(&db).is_ok());
    assert!(leading_zeros.as_u8(&db).is_ok());
    assert_eq!(f.as_u8(&db).unwrap(), 15);
    assert_eq!(lowercase.as_u8(&db).unwrap(), 255);
    assert_eq!(leading_zeros.as_u8(&db).unwrap(), 15);
}

#[test]
fn negative_numbers() {
    let db = RootDatabase::default();

    // Test negative numbers (only valid for signed types)
    let neg_small = Numeric::new(&db, Ident::from_slice(&db, "-1"), NumericKind::Signed);
    let neg_large = Numeric::new(&db, Ident::from_slice(&db, "-128"), NumericKind::Signed);

    // Should work for signed types
    assert!(neg_small.as_i8(&db).is_ok());
    assert!(neg_large.as_i8(&db).is_ok());
    assert!(neg_small.as_i16(&db).is_ok());
    assert!(neg_small.as_i32(&db).is_ok());
    assert!(neg_small.as_i64(&db).is_ok());

    // Should fail for unsigned types
    assert!(neg_small.as_u8(&db).is_err());
    assert!(neg_small.as_u16(&db).is_err());
    assert!(neg_small.as_u32(&db).is_err());
    assert!(neg_small.as_u64(&db).is_err());
}
