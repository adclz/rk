use db::WorkspaceDataBase;

use crate::{
    check::errors::e3_type::InferLiteralError,
    hir_def::{
        expressions::{
            expression::{Elementary, Integer, IntegerKind},
            spec::ElementarySpec,
        },
        interned::identifier::Ident,
    },
    hir_ty::ty::{InferType, Type},
};

use std::{f64, num::ParseIntError, u8};

// Figure 12 – Supported implicit type conversions

use time::{Date, Duration, PrimitiveDateTime, Time, macros::format_description};

impl<'db> Elementary {
    pub fn check(&self, db: &'db dyn WorkspaceDataBase) -> Result<(), InferLiteralError> {
        match self {
            Elementary::Date(dt) => dt
                .as_date(db)
                .map(|_| ())
                .map_err(|e| InferLiteralError::Invalid_DATE_Format(e.to_string())),
            Elementary::LDate(dt) => dt
                .as_long_date(db)
                .map(|_| ())
                .map_err(|e| InferLiteralError::Invalid_LDATE_Format(e.to_string())),
            Elementary::TimeOfDay(tod) => tod
                .as_tod(db)
                .map(|_| ())
                .map_err(|e| InferLiteralError::Invalid_TOD_Format(e.to_string())),
            Elementary::LTod(ltod) => ltod
                .as_long_tod(db)
                .map(|_| ())
                .map_err(|e| InferLiteralError::Invalid_LTOD_Format(e.to_string())),
            Elementary::DateAndTime(dt) => dt
                .as_date_time(db)
                .map(|_| ())
                .map_err(|e| InferLiteralError::Invalid_DT_Format(e.to_string())),
            Elementary::LDateTime(ldt) => ldt
                .as_long_date_time(db)
                .map(|_| ())
                .map_err(|e| InferLiteralError::Invalid_LDT_Format(e.to_string())),
            Elementary::Time(t) => t.as_time(db).map(|_| ()),
            Elementary::LTime(lt) => lt.as_ltime(db).map(|_| ()),
            Elementary::String(s) => s.as_single_string(db).map(|_| ()),
            Elementary::WString(s) => s.as_double_string(db).map(|_| ()),
            Elementary::Char(s) => {
                let bytes = s.as_single_string(db)?;
                if bytes.len() != 1 {
                    return Err(InferLiteralError::Invalid_CHAR_Length(bytes.len()));
                }
                Ok(())
            }
            Elementary::WChar(s) => {
                let chars = s.as_double_string(db)?;
                if chars.len() != 1 {
                    return Err(InferLiteralError::Invalid_WCHAR_Length(chars.len()));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl<'db> InferType {
    pub fn check_as(
        &self,
        db: &'db dyn WorkspaceDataBase,
        typ: ElementarySpec,
    ) -> Result<Type<'db>, InferLiteralError> {
        match typ {
            ElementarySpec::Bool | ElementarySpec::REDGEBool | ElementarySpec::FEDGEBool => {
                check_bool(db, self)
            }
            ElementarySpec::Byte => check_u8(db, self),
            ElementarySpec::Word => check_u16(db, self),
            ElementarySpec::DWord => check_u32(db, self),
            ElementarySpec::LWord => check_u64(db, self),
            ElementarySpec::USInt => check_u8(db, self),
            ElementarySpec::UInt => check_u16(db, self),
            ElementarySpec::UDInt => check_u32(db, self),
            ElementarySpec::ULInt => check_u64(db, self),
            ElementarySpec::SInt => check_i8(db, self),
            ElementarySpec::Int => check_i16(db, self),
            ElementarySpec::DInt => check_i32(db, self),
            ElementarySpec::LInt => check_i64(db, self),
            ElementarySpec::Real => check_f32(db, self),
            ElementarySpec::LReal => check_f64(db, self),
            _ => Err(InferLiteralError::TypeMismatch(format!(
                "cannot use numeric literal as {}",
                typ.type_name()
            ))),
        }
    }
}

fn check_bool<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_bool(db)
            .map(|_| Type::Elementary(ElementarySpec::Bool))
            .map_err(|err| InferLiteralError::Invalid_BOOL_Literal),
        _ => Err(InferLiteralError::Invalid_BOOL_Literal),
    }
}

fn check_u8<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_u8(db)
            .map(|_| Type::Elementary(ElementarySpec::USInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_UNSIGNED_8_BITS_Literal),
    }
}

fn check_u16<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_u16(db)
            .map(|_| Type::Elementary(ElementarySpec::UInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_UNSIGNED_16_BITS_Literal),
    }
}

fn check_u32<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_u32(db)
            .map(|_| Type::Elementary(ElementarySpec::UDInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_UNSIGNED_32_BITS_Literal),
    }
}

fn check_u64<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_u64(db)
            .map(|_| Type::Elementary(ElementarySpec::ULInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_UNSIGNED_64_BITS_Literal),
    }
}

fn check_i8<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_i8(db)
            .map(|_| Type::Elementary(ElementarySpec::SInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_SIGNED_8_BITS_Literal),
    }
}

fn check_i16<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_i16(db)
            .map(|_| Type::Elementary(ElementarySpec::Int))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_SIGNED_16_BITS_Literal),
    }
}

fn check_i32<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_i32(db)
            .map(|_| Type::Elementary(ElementarySpec::DInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_SIGNED_32_BITS_Literal),
    }
}

fn check_i64<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_i64(db)
            .map(|_| Type::Elementary(ElementarySpec::LInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_SIGNED_64_BITS_Literal),
    }
}

fn check_f32<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Float(real) => real
            .as_f32(db)
            .map(|_| Type::Elementary(ElementarySpec::Real))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        InferType::Integer(integer) => {
            let int_val = integer
                .as_i32(db)
                .map_err(|err| InferLiteralError::TypeMismatch(err.to_string()))?;
            // IEC standard allows integer to float conversion
            Ok(Type::Elementary(ElementarySpec::Real))
        }
    }
}

fn check_f64<'db>(
    db: &dyn WorkspaceDataBase,
    value: &InferType,
) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Float(real) => real
            .as_f64(db)
            .map(|_| Type::Elementary(ElementarySpec::LReal))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        InferType::Integer(integer) => {
            let int_val = integer
                .as_i64(db)
                .map_err(|err| InferLiteralError::TypeMismatch(err.to_string()))?;
            // IEC standard allows integer to float conversion
            Ok(Type::Elementary(ElementarySpec::LReal))
        }
    }
}

#[salsa::tracked]
impl Ident {
    #[salsa::tracked]
    pub fn as_bool(self, db: &dyn WorkspaceDataBase) -> Result<bool, std::str::ParseBoolError> {
        match self.text(db).to_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => other.parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u64(self, db: &dyn WorkspaceDataBase) -> Result<u64, std::num::ParseIntError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_f32(self, db: &dyn WorkspaceDataBase) -> Result<f32, std::num::ParseFloatError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_f64(self, db: &dyn WorkspaceDataBase) -> Result<f64, std::num::ParseFloatError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_date(self, db: &dyn WorkspaceDataBase) -> Result<Date, time::error::Parse> {
        let fmt = format_description!("[year]-[month]-[day]");
        Date::parse(
            &self
                .text(db)
                .to_uppercase()
                .replace("DATE#", "")
                .replace("D#", "")
                .replace('_', ""),
            &fmt,
        )
    }

    #[salsa::tracked]
    pub fn as_long_date(self, db: &dyn WorkspaceDataBase) -> Result<Date, time::error::Parse> {
        let fmt = format_description!("[year]-[month]-[day]");
        Date::parse(
            &self
                .text(db)
                .to_uppercase()
                .replace("LDATE#", "")
                .replace("LD#", "")
                .replace('_', ""),
            &fmt,
        )
    }

    #[salsa::tracked]
    pub fn as_tod(self, db: &dyn WorkspaceDataBase) -> Result<Time, time::error::Parse> {
        let fmt = format_description!("[hour]:[minute]:[second].[subsecond]");
        Time::parse(
            &self
                .text(db)
                .to_uppercase()
                .replace("TIME_OF_DAY#", "")
                .replace("TOD#", "")
                .replace('_', ""),
            &fmt,
        )
    }

    #[salsa::tracked]
    pub fn as_long_tod(self, db: &dyn WorkspaceDataBase) -> Result<Time, time::error::Parse> {
        let fmt = format_description!("[hour]:[minute]:[second].[subsecond]");
        Time::parse(
            &self
                .text(db)
                .to_uppercase()
                .replace("LTIME_OF_DAY#", "")
                .replace("LTOD#", "")
                .replace('_', ""),
            &fmt,
        )
    }

    #[salsa::tracked]
    pub fn as_date_time(
        self,
        db: &dyn WorkspaceDataBase,
    ) -> Result<PrimitiveDateTime, time::error::Parse> {
        let fmt = format_description!("[year]-[month]-[day]-[hour]:[minute]:[second].[subsecond]");
        PrimitiveDateTime::parse(
            &self
                .text(db)
                .to_uppercase()
                .replace("DATE_AND_TIME#", "")
                .replace("DT#", "")
                .replace('_', ""),
            &fmt,
        )
    }

    #[salsa::tracked]
    pub fn as_long_date_time(
        self,
        db: &dyn WorkspaceDataBase,
    ) -> Result<PrimitiveDateTime, time::error::Parse> {
        let fmt = format_description!("[year]-[month]-[day]-[hour]:[minute]:[second].[subsecond]");
        PrimitiveDateTime::parse(
            &self
                .text(db)
                .to_uppercase()
                .replace("LDATE_AND_TIME#", "")
                .replace("LDT#", "")
                .replace('_', ""),
            &fmt,
        )
    }

    #[salsa::tracked]
    pub fn as_single_string(
        self,
        db: &dyn WorkspaceDataBase,
    ) -> Result<Vec<u8>, InferLiteralError> {
        parse_single_byte_string(&self.text(db).replace("STRING#", ""))
    }

    #[salsa::tracked]
    pub fn as_double_string(
        self,
        db: &dyn WorkspaceDataBase,
    ) -> Result<Vec<char>, InferLiteralError> {
        parse_double_byte_string(&self.text(db).replace("WSTRING#", ""))
    }

    #[salsa::tracked]
    pub fn as_time(self, db: &dyn WorkspaceDataBase) -> Result<Duration, InferLiteralError> {
        let text = self.text(db).to_uppercase();
        let value = text.split_once('#').map_or(text.as_str(), |(_, v)| v);
        parse_duration_components(value, "TIME")
    }

    #[salsa::tracked]
    pub fn as_ltime(self, db: &dyn WorkspaceDataBase) -> Result<Duration, InferLiteralError> {
        let text = self.text(db).to_uppercase();
        let value = text.split_once('#').map_or(text.as_str(), |(_, v)| v);
        parse_duration_components(value, "LTIME")
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UnsignedIntError {
    ParseIntError(ParseIntError),
    NegativeSign,
}

impl From<ParseIntError> for UnsignedIntError {
    fn from(err: ParseIntError) -> Self {
        UnsignedIntError::ParseIntError(err)
    }
}

impl std::error::Error for UnsignedIntError {}

impl std::fmt::Display for UnsignedIntError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnsignedIntError::ParseIntError(err) => write!(f, "{err}"),
            UnsignedIntError::NegativeSign => write!(f, "literal can not be negative"),
        }
    }
}

fn check_sign(s: &str) -> Result<&str, UnsignedIntError> {
    if s.starts_with('-') {
        Err(UnsignedIntError::NegativeSign)
    } else {
        Ok(s)
    }
}

#[salsa::tracked]
impl Integer {
    #[salsa::tracked]
    pub fn as_bool(self, db: &dyn WorkspaceDataBase) -> Result<bool, std::str::ParseBoolError> {
        match self.ident(db).text(db).to_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => other.parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u8(self, db: &dyn WorkspaceDataBase) -> Result<u8, UnsignedIntError> {
        match self.kind(db) {
            IntegerKind::Binary => u8::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("2#"))?,
                2,
            ),
            IntegerKind::Octal => u8::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("8#"))?,
                8,
            ),
            IntegerKind::Hex => u8::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("16#"))?,
                16,
            ),
            IntegerKind::Signed => check_sign(self.ident(db).text(db))?.parse(),
        }
        .map_err(|err| err.into())
    }

    #[salsa::tracked]
    pub fn as_u16(self, db: &dyn WorkspaceDataBase) -> Result<u16, UnsignedIntError> {
        match self.kind(db) {
            IntegerKind::Binary => u16::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("2#"))?,
                2,
            ),
            IntegerKind::Octal => u16::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("8#"))?,
                8,
            ),
            IntegerKind::Hex => u16::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("16#"))?,
                16,
            ),
            IntegerKind::Signed => check_sign(self.ident(db).text(db))?.parse(),
        }
        .map_err(|err| err.into())
    }

    #[salsa::tracked]
    pub fn as_u32(self, db: &dyn WorkspaceDataBase) -> Result<u32, UnsignedIntError> {
        match self.kind(db) {
            IntegerKind::Binary => u32::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("2#"))?,
                2,
            ),
            IntegerKind::Octal => u32::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("8#"))?,
                8,
            ),
            IntegerKind::Hex => u32::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("16#"))?,
                16,
            ),
            IntegerKind::Signed => check_sign(self.ident(db).text(db))?.parse(),
        }
        .map_err(|err| err.into())
    }

    #[salsa::tracked]
    pub fn as_u64(self, db: &dyn WorkspaceDataBase) -> Result<u64, UnsignedIntError> {
        match self.kind(db) {
            IntegerKind::Binary => u64::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("2#"))?,
                2,
            ),
            IntegerKind::Octal => u64::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("8#"))?,
                8,
            ),
            IntegerKind::Hex => u64::from_str_radix(
                check_sign(self.ident(db).text(db).trim_start_matches("16#"))?,
                16,
            ),
            IntegerKind::Signed => check_sign(self.ident(db).text(db))?.parse(),
        }
        .map_err(|err| err.into())
    }

    #[salsa::tracked]
    pub fn as_i8(self, db: &dyn WorkspaceDataBase) -> Result<i8, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                i8::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                i8::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                i8::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i16(self, db: &dyn WorkspaceDataBase) -> Result<i16, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                i16::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                i16::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                i16::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i32(self, db: &dyn WorkspaceDataBase) -> Result<i32, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                i32::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                i32::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                i32::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i64(self, db: &dyn WorkspaceDataBase) -> Result<i64, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                i64::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                i64::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                i64::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }
}

pub fn parse_single_byte_string(s: &str) -> Result<Vec<u8>, InferLiteralError> {
    let inner = &s[1..s.len() - 1];
    let mut result = Vec::new();
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let h1 = chars
                .next()
                .ok_or(InferLiteralError::Incomplete_STRING_XX_Escape)?;
            let h2 = chars
                .next()
                .ok_or(InferLiteralError::Incomplete_STRING_XX_Escape)?;
            let hex = format!("{h1}{h2}");
            let byte = u8::from_str_radix(&hex, 16)
                .map_err(|_| InferLiteralError::Incomplete_STRING_XX_Escape)?;
            result.push(byte);
        } else {
            // Regular single-byte character
            if (c as u32) > 0xFF {
                return Err(InferLiteralError::Invalid_STRING_CHAR(c.to_string()));
            }
            result.push(c as u8);
        }
    }
    Ok(result)
}

pub fn parse_double_byte_string(s: &str) -> Result<Vec<char>, InferLiteralError> {
    let inner = &s[1..s.len() - 1];
    let mut result = Vec::new();
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let h1 = chars
                .next()
                .ok_or(InferLiteralError::Incomplete_WSTRING_XXXX_Escape)?;
            let h2 = chars
                .next()
                .ok_or(InferLiteralError::Incomplete_WSTRING_XXXX_Escape)?;
            let h3 = chars
                .next()
                .ok_or(InferLiteralError::Incomplete_WSTRING_XXXX_Escape)?;
            let h4 = chars
                .next()
                .ok_or(InferLiteralError::Incomplete_WSTRING_XXXX_Escape)?;
            let hex = format!("{h1}{h2}{h3}{h4}");
            let code = u16::from_str_radix(&hex, 16)
                .map_err(|_| InferLiteralError::Invalid_WSTRING_Hex_Escape(hex.to_string()))?;
            result.push(char::from_u32(code as u32).ok_or_else(|| {
                InferLiteralError::Invalid_WSTRING_Unicode_Scalar(hex.to_string())
            })?);
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

fn parse_duration_components(s: &str, kind: &'static str) -> Result<Duration, InferLiteralError> {
    // Remove underscores (allowed in literals per IEC 61131-3)
    let cleaned = s.replace('_', "");

    // Handle optional leading sign (applies to the whole duration)
    let (is_negative, value_str) = if cleaned.starts_with('-') {
        (true, &cleaned[1..])
    } else if cleaned.starts_with('+') {
        (false, &cleaned[1..])
    } else {
        (false, cleaned.as_str())
    };

    if value_str.is_empty() {
        return Err(InferLiteralError::Invalid_TIME_Components);
    }

    let mut total_nanos = 0i64;
    let mut remaining = value_str;

    // Parse each component (days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds)
    while !remaining.is_empty() {
        let (value, unit, rest, is_decimal) = parse_next_component(remaining, kind)?;

        let nanos = if is_decimal {
            // Value is already converted to nanoseconds
            value
        } else {
            // Integer value, needs conversion
            match unit {
                "D" => value * 24 * 60 * 60 * 1_000_000_000,
                "H" => value * 60 * 60 * 1_000_000_000,
                "M" => value * 60 * 1_000_000_000,
                "S" => value * 1_000_000_000,
                "MS" => value * 1_000_000,
                "US" => value * 1_000,
                "NS" => value,
                _ => {
                    return Err(InferLiteralError::Invalid_TIME_Unit(unit.to_string()));
                }
            }
        };

        total_nanos = total_nanos
            .checked_add(nanos)
            .ok_or(InferLiteralError::DurationOverflow)?;

        remaining = rest;
    }

    let total_nanos = if is_negative {
        -total_nanos
    } else {
        total_nanos
    };
    Ok(Duration::nanoseconds(total_nanos))
}

fn parse_next_component<'a>(
    s: &'a str,
    kind: &'static str,
) -> Result<(i64, &'a str, &'a str, bool), InferLiteralError> {
    let mut number_end = 0;
    let mut found_decimal = false;

    // Find the end of the number (digits and at most one decimal point)
    // Input is already uppercased; sign is handled at the caller level
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            number_end = i + 1;
        } else if c == '.' && !found_decimal {
            found_decimal = true;
            number_end = i + 1;
        } else {
            break;
        }
    }

    if number_end == 0 {
        return Err(InferLiteralError::ExpectedNumber);
    }

    let number_str = &s[..number_end];
    let remainder = &s[number_end..];

    // Find the unit (already uppercased)
    let mut unit_end = 0;
    for (i, c) in remainder.char_indices() {
        if c.is_ascii_alphabetic() {
            unit_end = i + 1;
        } else {
            break;
        }
    }

    if unit_end == 0 {
        return Err(InferLiteralError::Invalid_TIME_Unit(kind.to_string()));
    }

    let unit = &remainder[..unit_end];
    let rest = &remainder[unit_end..];

    // Parse the number and convert to nanoseconds
    let value_nanos = if found_decimal {
        let float_val: f64 = number_str
            .parse()
            .map_err(|_| InferLiteralError::InvalidNumber(number_str.to_string()))?;

        let nanos = match unit {
            "D" => float_val * 24.0 * 60.0 * 60.0 * 1_000_000_000.0,
            "H" => float_val * 60.0 * 60.0 * 1_000_000_000.0,
            "M" => float_val * 60.0 * 1_000_000_000.0,
            "S" => float_val * 1_000_000_000.0,
            "MS" => float_val * 1_000_000.0,
            "US" => float_val * 1_000.0,
            "NS" => float_val,
            _ => return Err(InferLiteralError::Invalid_TIME_Unit(unit.to_string())),
        };

        nanos as i64
    } else {
        let int_val: i64 = number_str
            .parse()
            .map_err(|_| InferLiteralError::InvalidNumber(number_str.to_string()))?;
        int_val
    };

    Ok((value_nanos, unit, rest, found_decimal))
}
