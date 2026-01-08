use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::literals::InferLiteralError,
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
    pub fn check(&self, db: &'db dyn BaseDatabase) -> Result<(), InferLiteralError> {
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
            _ => Ok(()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExplicitCast<'db> {
    from: Type<'db>,
    to: Type<'db>,
}

impl<'db> ElementarySpec {
    /// Explicit casts according to IEC 61131-3 standard
    ///
    /// See 6.6.1.6 Data type conversion
    pub fn explicit_cast(&self, typ: ElementarySpec) -> Option<ExplicitCast<'db>> {
        use ElementarySpec::*;
        match typ {
            LReal => match self {
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(LReal),
                    })
                }
                _ => None,
            },
            Real => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(Real),
                    })
                }
                _ => None,
            },
            LInt => match self {
                LReal | Real | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord
                | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(LInt),
                }),
                _ => None,
            },
            DInt => match self {
                Real | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(DInt),
                    })
                }
                _ => None,
            },
            Int => match self {
                SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(Int),
                    })
                }
                _ => None,
            },
            SInt => match self {
                ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(SInt),
                }),
                _ => None,
            },
            ULInt => match self {
                LReal | Real | LInt | DInt | Int | SInt | UDInt | UInt | USInt | LWord | DWord
                | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(ULInt),
                }),
                _ => None,
            },
            UDInt => match self {
                Real | DInt | Int | SInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(UDInt),
                    })
                }
                _ => None,
            },
            UInt => match self {
                Int | SInt | USInt | LWord | DWord | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(UInt),
                }),
                _ => None,
            },
            USInt => match self {
                SInt | LWord | DWord | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(USInt),
                }),
                _ => None,
            },
            LWord => match self {
                LReal | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord | Word
                | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(LWord),
                }),
                _ => None,
            },
            DWord => match self {
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(DWord),
                    })
                }
                _ => None,
            },
            Word => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(Word),
                    })
                }
                _ => None,
            },
            Byte => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(Byte),
                }),
                _ => None,
            },
            Bool | REDGEBool | FEDGEBool => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(*self),
                }),
                _ => None,
            },
            _ => None,
        }
    }

    /// Implicit casts according to IEC 61131-3 standard
    ///
    /// See 6.6.1.6 Data type conversion
    pub fn implicit_cast(&self, typ: ElementarySpec) -> Option<ElementarySpec> {
        use ElementarySpec::*;

        Some(match typ {
            // BOOL BYTE WORD DWORD LWORD
            Bool | REDGEBool | FEDGEBool => match self {
                Byte => Byte,
                Word => Word,
                DWord => DWord,
                LWord => LWord,
                _ => None?,
            },
            Byte => match self {
                Word => Word,
                DWord => DWord,
                LWord => LWord,
                _ => None?,
            },
            Word => match self {
                DWord => DWord,
                LWord => LWord,
                _ => None?,
            },
            DWord => match self {
                LWord => LWord,
                _ => None?,
            },
            // SINT INT DINT LINT
            SInt => match self {
                Int => Int,
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            Int => match self {
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            // DINT LREAL
            DInt => match self {
                LInt => LInt,
                // no real (see table in standard)
                LReal => LReal,
                _ => None?,
            },
            // REAL LREAL
            Real => match self {
                LReal => Real,
                _ => None?,
            },
            // USINT UINT UDINT ULINT
            USInt => match self {
                UInt => UInt,
                UDInt => UDInt,
                ULInt => ULInt,
                // sint is explicit only
                Int => Int,
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            UInt => match self {
                UDInt => UDInt,
                ULInt => ULInt,
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            UDInt => match self {
                LReal => LReal,
                LInt => LInt,
                ULInt => ULInt,
                _ => None?,
            },
            // TIME LTIME
            Time => match self {
                LTime => LTime,
                _ => None?,
            },
            // DT LDT
            DateAndTime => match self {
                LDateTime => LDateTime,
                _ => None?,
            },
            // DATE LDATE
            Date => match self {
                LDate => LDate,
                _ => None?,
            },
            // TOD LTOD
            Tod => match self {
                LTod => LTod,
                _ => None?,
            },
            _ => None?,
        })
    }
}

impl<'db> InferType {
    pub fn check_as(
        &self,
        db: &'db dyn BaseDatabase,
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
            _ => todo!(),
        }
    }
}

fn check_bool<'db>(
    db: &dyn BaseDatabase,
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

fn check_u8<'db>(db: &dyn BaseDatabase, value: &InferType) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_u8(db)
            .map(|_| Type::Elementary(ElementarySpec::USInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_UNSIGNED_8_BITS_Literal),
    }
}

fn check_u16<'db>(
    db: &dyn BaseDatabase,
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
    db: &dyn BaseDatabase,
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
    db: &dyn BaseDatabase,
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

fn check_i8<'db>(db: &dyn BaseDatabase, value: &InferType) -> Result<Type<'db>, InferLiteralError> {
    match value {
        InferType::Integer(n) => n
            .as_i8(db)
            .map(|_| Type::Elementary(ElementarySpec::SInt))
            .map_err(|err| InferLiteralError::TypeMismatch(err.to_string())),
        _ => Err(InferLiteralError::Invalid_SIGNED_8_BITS_Literal),
    }
}

fn check_i16<'db>(
    db: &dyn BaseDatabase,
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
    db: &dyn BaseDatabase,
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
    db: &dyn BaseDatabase,
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
    db: &dyn BaseDatabase,
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
    db: &dyn BaseDatabase,
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
    pub fn as_bool(self, db: &dyn BaseDatabase) -> Result<bool, std::str::ParseBoolError> {
        match self.text(db).to_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => other.parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u64(self, db: &dyn BaseDatabase) -> Result<u64, std::num::ParseIntError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_f32(self, db: &dyn BaseDatabase) -> Result<f32, std::num::ParseFloatError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_f64(self, db: &dyn BaseDatabase) -> Result<f64, std::num::ParseFloatError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_date(self, db: &dyn BaseDatabase) -> Result<Date, time::error::Parse> {
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
    pub fn as_long_date(self, db: &dyn BaseDatabase) -> Result<Date, time::error::Parse> {
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
    pub fn as_tod(self, db: &dyn BaseDatabase) -> Result<Time, time::error::Parse> {
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
    pub fn as_long_tod(self, db: &dyn BaseDatabase) -> Result<Time, time::error::Parse> {
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
        db: &dyn BaseDatabase,
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
        db: &dyn BaseDatabase,
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
    pub fn as_single_string(self, db: &dyn BaseDatabase) -> Result<Vec<u8>, InferLiteralError> {
        parse_single_byte_string(&self.text(db).replace("STRING#", ""))
    }

    #[salsa::tracked]
    pub fn as_double_string(self, db: &dyn BaseDatabase) -> Result<Vec<char>, InferLiteralError> {
        parse_double_byte_string(&self.text(db).replace("WSTRING#", ""))
    }

    #[salsa::tracked]
    pub fn as_time(self, db: &dyn BaseDatabase) -> Result<Duration, InferLiteralError> {
        parse_duration_components(
            self.text(db)
                .to_uppercase()
                .replace("TIME#", "")
                .replace("T#", "")
                .as_str(),
            "TIME",
        )
    }

    #[salsa::tracked]
    pub fn as_ltime(self, db: &dyn BaseDatabase) -> Result<Duration, InferLiteralError> {
        parse_duration_components(
            &self
                .text(db)
                .to_uppercase()
                .replace("LTIME#", "")
                .replace("LT#", ""),
            "LT#",
        )
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
    pub fn as_bool(self, db: &dyn BaseDatabase) -> Result<bool, std::str::ParseBoolError> {
        match self.ident(db).text(db).to_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => other.parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u8(self, db: &dyn BaseDatabase) -> Result<u8, UnsignedIntError> {
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
    pub fn as_u16(self, db: &dyn BaseDatabase) -> Result<u16, UnsignedIntError> {
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
    pub fn as_u32(self, db: &dyn BaseDatabase) -> Result<u32, UnsignedIntError> {
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
    pub fn as_u64(self, db: &dyn BaseDatabase) -> Result<u64, UnsignedIntError> {
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
    pub fn as_i8(self, db: &dyn BaseDatabase) -> Result<i8, std::num::ParseIntError> {
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
    pub fn as_i16(self, db: &dyn BaseDatabase) -> Result<i16, std::num::ParseIntError> {
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
    pub fn as_i32(self, db: &dyn BaseDatabase) -> Result<i32, std::num::ParseIntError> {
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
    pub fn as_i64(self, db: &dyn BaseDatabase) -> Result<i64, std::num::ParseIntError> {
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

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        match self.kind(db) {
            IntegerKind::Binary => format!("[Binary] {}", self.ident(db).text(db)),
            IntegerKind::Hex => format!("[Hexa] {}", self.ident(db).text(db)),
            IntegerKind::Octal => format!("[Octal] {}", self.ident(db).text(db)),
            IntegerKind::Signed => self.ident(db).text(db).to_string(),
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
    let mut total_nanos = 0i64;
    let mut remaining = s;

    // Remove underscores (allowed in literals)
    let cleaned = remaining.replace('_', "");
    remaining = &cleaned;

    // Parse each component (days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds)
    while !remaining.is_empty() {
        let (value, unit, rest, is_decimal) = parse_next_component(remaining, kind)?;

        let nanos = if is_decimal {
            // Value is already converted to nanoseconds
            value
        } else {
            // Integer value, needs conversion
            match unit {
                "d" | "D" => value * 24 * 60 * 60 * 1_000_000_000,
                "h" | "H" => value * 60 * 60 * 1_000_000_000,
                "m" | "M" => value * 60 * 1_000_000_000,
                "s" | "S" => value * 1_000_000_000,
                "ms" | "MS" => value * 1_000_000,
                "us" | "US" => value * 1_000,
                "ns" | "NS" => value,
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

    if s.is_empty() {
        return Err(InferLiteralError::Invalid_TIME_Components);
    }

    Ok(Duration::nanoseconds(total_nanos))
}

fn parse_next_component<'a>(
    s: &'a str,
    kind: &'static str,
) -> Result<(i64, &'a str, &'a str, bool), InferLiteralError> {
    let mut number_end = 0;
    let mut found_decimal = false;

    // Find the end of the number (including decimal point and negative sign)
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            number_end = i + 1;
        } else if c == '.' && !found_decimal {
            found_decimal = true;
            number_end = i + 1;
        } else if c == '-' && i == 0 {
            // Allow negative sign only at the beginning
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

    // Find the unit
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

    // Parse the number (handle decimals)
    let value_nanos = if found_decimal {
        let float_val: f64 = number_str
            .parse()
            .map_err(|_| InferLiteralError::InvalidNumber(number_str.to_string()))?;

        // Convert to nanoseconds based on unit, then truncate to u64
        let nanos = match unit.to_uppercase().as_str() {
            "D" => float_val * 24.0 * 60.0 * 60.0 * 1_000_000_000.0,
            "H" => float_val * 60.0 * 60.0 * 1_000_000_000.0,
            "M" => float_val * 60.0 * 1_000_000_000.0,
            "S" => float_val * 1_000_000_000.0,
            "MS" => float_val * 1_000_000.0,
            "US" => float_val * 1_000.0,
            "NS" => float_val,
            _ => {
                return Err(InferLiteralError::Invalid_TIME_Unit(unit.to_string()));
            }
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
