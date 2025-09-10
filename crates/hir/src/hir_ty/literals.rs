use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::sem_errors::LitCheckError,
    hir_def::{
        expressions::{
            expression::{Elementary, Integer, IntegerKind},
            spec::ElementarySpec,
        },
        interned::identifier::Ident,
    },
};

use time::{Date, Duration, PrimitiveDateTime, Time, macros::format_description};

impl<'db> ElementarySpec {
    pub fn lit_check(
        &self,
        db: &'db dyn BaseDatabase,
        spec: Elementary,
    ) -> Result<(), LitCheckError> {
        match self {
            ElementarySpec::Bool | ElementarySpec::REDGEBool | ElementarySpec::FEDGEBool => {
                check_bool(db, &spec)
            }
            ElementarySpec::Byte => check_u8(db, &spec),
            ElementarySpec::Word => check_u16(db, &spec),
            ElementarySpec::DWord => check_u32(db, &spec),
            ElementarySpec::LWord => check_u64(db, &spec),
            ElementarySpec::USInt => check_u8(db, &spec),
            ElementarySpec::UInt => check_u16(db, &spec),
            ElementarySpec::UDInt => check_u32(db, &spec),
            ElementarySpec::ULInt => check_u64(db, &spec),
            ElementarySpec::SInt => check_i8(db, &spec),
            ElementarySpec::Int => check_i16(db, &spec),
            ElementarySpec::DInt => check_i32(db, &spec),
            ElementarySpec::LInt => check_i64(db, &spec),
            ElementarySpec::Real => check_f32(db, &spec),
            ElementarySpec::LReal => check_f64(db, &spec),
            ElementarySpec::Date => match spec {
                Elementary::InferIdent(ident) => {
                    ident
                        .as_date(db)
                        .map(|_| ())
                        .map_err(|e| LitCheckError::InvalidFormat {
                            kind: "DATE",
                            msg: e.to_string(),
                        })
                }
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a date literal".into(),
                )),
            },
            ElementarySpec::LDate => match spec {
                Elementary::InferIdent(ident) => {
                    ident
                        .as_long_date(db)
                        .map(|_| ())
                        .map_err(|e| LitCheckError::InvalidFormat {
                            kind: "LDATE",
                            msg: e.to_string(),
                        })
                }
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a long date literal".into(),
                )),
            },
            ElementarySpec::Tod => match spec {
                Elementary::InferIdent(ident) => {
                    ident
                        .as_tod(db)
                        .map(|_| ())
                        .map_err(|e| LitCheckError::InvalidFormat {
                            kind: "TIME_OF_DAY",
                            msg: e.to_string(),
                        })
                }
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a time-of-day literal".into(),
                )),
            },
            ElementarySpec::LTod => match spec {
                Elementary::InferIdent(ident) => {
                    ident
                        .as_long_tod(db)
                        .map(|_| ())
                        .map_err(|e| LitCheckError::InvalidFormat {
                            kind: "LTIME_OF_DAY",
                            msg: e.to_string(),
                        })
                }
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a long time-of-day literal".into(),
                )),
            },
            ElementarySpec::Dt => match spec {
                Elementary::InferIdent(ident) => ident
                    .as_date_time(db)
                    .map_err(|e| LitCheckError::InvalidFormat {
                        kind: "DATE_AND_TIME",
                        msg: e.to_string(),
                    })
                    .map(|_| ()),
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a date-and-time literal".into(),
                )),
            },

            // 6a/b LDT / LDATE_AND_TIME
            ElementarySpec::Ldt => match spec {
                Elementary::InferIdent(ident) => ident
                    .as_long_date_time(db)
                    .map_err(|e| LitCheckError::InvalidFormat {
                        kind: "LDATE_AND_TIME",
                        msg: e.to_string(),
                    })
                    .map(|_| ()),
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a long date-and-time literal".into(),
                )),
            },
            ElementarySpec::Time => match spec {
                Elementary::InferIdent(ident) => ident.as_time(db).map(|_| ()),
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a time duration literal".into(),
                )),
            },
            ElementarySpec::LTime => match spec {
                Elementary::InferIdent(ident) => ident.as_ltime(db).map(|_| ()),
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a long time duration literal".into(),
                )),
            },

            ElementarySpec::String => match spec {
                Elementary::InferIdent(ident) => ident.as_single_string(db).map(|_| ()),
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a string literal".into(),
                )),
            },
            ElementarySpec::WString => match spec {
                Elementary::InferIdent(ident) => ident.as_double_string(db).map(|_| ()),
                _ => Err(LitCheckError::TypeMismatch(
                    "expected a wide string literal".into(),
                )),
            },
            ElementarySpec::Char => todo!(),
            ElementarySpec::WChar => todo!(),
        }
    }
}

fn check_bool(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::InferIdent(n) => n
            .as_bool(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        Elementary::InferInteger(n) => n
            .as_bool(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "Expected one of '0' | '1' | 'TRUE' | 'FALSE'".into(),
        )),
    }
}

fn check_u8(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::InferInteger(n) => n
            .as_u8(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected an unsigned 8-bit integer".into(),
        )),
    }
}

fn check_u16(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::Word(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::Int(n)
        | Elementary::UInt(n)
        | Elementary::InferInteger(n) => n
            .as_u16(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected an unsigned 16-bit integer".into(),
        )),
    }
}

fn check_u32(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::Word(n)
        | Elementary::DWord(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::DInt(n)
        | Elementary::Int(n)
        | Elementary::UInt(n)
        | Elementary::UDInt(n)
        | Elementary::InferInteger(n) => n
            .as_u32(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected an unsigned 32-bit integer".into(),
        )),
    }
}

fn check_u64(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::Word(n)
        | Elementary::DWord(n)
        | Elementary::LWord(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::DInt(n)
        | Elementary::Int(n)
        | Elementary::LInt(n)
        | Elementary::UInt(n)
        | Elementary::UDInt(n)
        | Elementary::ULInt(n)
        | Elementary::InferInteger(n) => n
            .as_u64(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected an unsigned 64-bit integer".into(),
        )),
    }
}

fn check_i8(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::InferInteger(n) => n
            .as_i8(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected a signed 8-bit integer".into(),
        )),
    }
}

fn check_i16(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::Word(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::Int(n)
        | Elementary::UInt(n)
        | Elementary::InferInteger(n) => n
            .as_i16(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected a signed 16-bit integer".into(),
        )),
    }
}

fn check_i32(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::Word(n)
        | Elementary::DWord(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::DInt(n)
        | Elementary::Int(n)
        | Elementary::UInt(n)
        | Elementary::UDInt(n)
        | Elementary::InferInteger(n) => n
            .as_i32(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected a signed 32-bit integer".into(),
        )),
    }
}

fn check_i64(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::Byte(n)
        | Elementary::Word(n)
        | Elementary::DWord(n)
        | Elementary::LWord(n)
        | Elementary::SInt(n)
        | Elementary::USInt(n)
        | Elementary::DInt(n)
        | Elementary::Int(n)
        | Elementary::LInt(n)
        | Elementary::UInt(n)
        | Elementary::UDInt(n)
        | Elementary::ULInt(n)
        | Elementary::InferInteger(n) => n
            .as_i64(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected a signed 64-bit integer".into(),
        )),
    }
}

fn check_f32(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::InferIdent(ident) => ident
            .as_f32(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected a 32-bit floating point number".into(),
        )),
    }
}

fn check_f64(db: &dyn BaseDatabase, value: &Elementary) -> Result<(), LitCheckError> {
    match value {
        Elementary::InferIdent(ident) => ident
            .as_f64(db)
            .map(|_| ())
            .map_err(|err| LitCheckError::TypeMismatch(err.to_string())),
        _ => Err(LitCheckError::TypeMismatch(
            "expected a 64-bit floating point number".into(),
        )),
    }
}

#[salsa::tracked]
impl Ident {
    #[salsa::tracked]
    pub fn as_bool(self, db: &dyn BaseDatabase) -> Result<bool, std::str::ParseBoolError> {
        self.text(db).to_lowercase().parse()
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
    pub fn as_single_string(self, db: &dyn BaseDatabase) -> Result<Vec<u8>, LitCheckError> {
        parse_single_byte_string(&self.text(db).replace("STRING#", ""))
    }

    #[salsa::tracked]
    pub fn as_double_string(self, db: &dyn BaseDatabase) -> Result<Vec<char>, LitCheckError> {
        parse_double_byte_string(&self.text(db).replace("WSTRING#", ""))
    }

    #[salsa::tracked]
    pub fn as_time(self, db: &dyn BaseDatabase) -> Result<Duration, LitCheckError> {
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
    pub fn as_ltime(self, db: &dyn BaseDatabase) -> Result<Duration, LitCheckError> {
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

#[salsa::tracked]
impl Integer {
    #[salsa::tracked]
    pub fn as_bool(self, db: &dyn BaseDatabase) -> Result<bool, std::str::ParseBoolError> {
        match self.ident(db).text(db).to_lowercase().as_str() {
            "1" => Ok(true),
            "0" => Ok(false),
            other => other.parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u8(self, db: &dyn BaseDatabase) -> Result<u8, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                u8::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                u8::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                u8::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u16(self, db: &dyn BaseDatabase) -> Result<u16, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                u16::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                u16::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                u16::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u32(self, db: &dyn BaseDatabase) -> Result<u32, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                u32::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                u32::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                u32::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u64(self, db: &dyn BaseDatabase) -> Result<u64, std::num::ParseIntError> {
        match self.kind(db) {
            IntegerKind::Binary => {
                u64::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            IntegerKind::Octal => {
                u64::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            IntegerKind::Hex => {
                u64::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            IntegerKind::Signed => self.ident(db).text(db).parse(),
        }
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

pub fn parse_single_byte_string(s: &str) -> Result<Vec<u8>, LitCheckError> {
    let inner = &s[1..s.len() - 1];
    let mut result = Vec::new();
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let h1 = chars.next().ok_or_else(|| LitCheckError::InvalidFormat {
                kind: "STRING",
                msg: "Incomplete $xx escape".into(),
            })?;
            let h2 = chars.next().ok_or_else(|| LitCheckError::InvalidFormat {
                kind: "STRING",
                msg: "Incomplete $xx escape".into(),
            })?;
            let hex = format!("{h1}{h2}");
            let byte = u8::from_str_radix(&hex, 16).map_err(|_| LitCheckError::InvalidFormat {
                kind: "STRING",
                msg: format!("Invalid hex escape ${hex}"),
            })?;
            result.push(byte);
        } else {
            // Regular single-byte character
            if (c as u32) > 0xFF {
                return Err(LitCheckError::InvalidFormat {
                    kind: "STRING",
                    msg: format!("Character {c} not allowed in single-byte string"),
                });
            }
            result.push(c as u8);
        }
    }
    Ok(result)
}

pub fn parse_double_byte_string(s: &str) -> Result<Vec<char>, LitCheckError> {
    let inner = &s[1..s.len() - 1];
    let mut result = Vec::new();
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let h1 = chars.next().ok_or_else(|| LitCheckError::InvalidFormat {
                kind: "WSTRING",
                msg: "Incomplete $xxxx escape".into(),
            })?;
            let h2 = chars.next().ok_or_else(|| LitCheckError::InvalidFormat {
                kind: "WSTRING",
                msg: "Incomplete $xxxx escape".into(),
            })?;
            let h3 = chars.next().ok_or_else(|| LitCheckError::InvalidFormat {
                kind: "WSTRING",
                msg: "Incomplete $xxxx escape".into(),
            })?;
            let h4 = chars.next().ok_or_else(|| LitCheckError::InvalidFormat {
                kind: "WSTRING",
                msg: "Incomplete $xxxx escape".into(),
            })?;
            let hex = format!("{h1}{h2}{h3}{h4}");
            let code = u16::from_str_radix(&hex, 16).map_err(|_| LitCheckError::InvalidFormat {
                kind: "WSTRING",
                msg: format!("Invalid hex escape ${hex}"),
            })?;
            result.push(char::from_u32(code as u32).ok_or_else(|| {
                LitCheckError::InvalidFormat {
                    kind: "WSTRING",
                    msg: format!("Invalid Unicode scalar: ${hex}"),
                }
            })?);
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

fn parse_duration_components(s: &str, kind: &'static str) -> Result<Duration, LitCheckError> {
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
                    return Err(LitCheckError::InvalidFormat {
                        kind,
                        msg: format!("Invalid time unit: {unit}"),
                    });
                }
            }
        };

        total_nanos = total_nanos
            .checked_add(nanos)
            .ok_or_else(|| LitCheckError::OutOfRange("Duration overflow".into()))?;

        remaining = rest;
    }

    if total_nanos == 0 && !s.is_empty() {
        return Err(LitCheckError::InvalidFormat {
            kind,
            msg: "No valid time components found".into(),
        });
    }

    Ok(Duration::nanoseconds(total_nanos))
}

fn parse_next_component<'a>(
    s: &'a str,
    kind: &'static str,
) -> Result<(i64, &'a str, &'a str, bool), LitCheckError> {
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
        return Err(LitCheckError::InvalidFormat {
            kind,
            msg: "Expected number".into(),
        });
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
        return Err(LitCheckError::InvalidFormat {
            kind,
            msg: "Expected time unit".into(),
        });
    }

    let unit = &remainder[..unit_end];
    let rest = &remainder[unit_end..];

    // Parse the number (handle decimals)
    let value_nanos = if found_decimal {
        let float_val: f64 = number_str
            .parse()
            .map_err(|_| LitCheckError::InvalidFormat {
                kind,
                msg: format!("Invalid number: {number_str}"),
            })?;

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
                return Err(LitCheckError::InvalidFormat {
                    kind,
                    msg: format!("Invalid time unit: {unit}"),
                });
            }
        };

        nanos as i64
    } else {
        let int_val: i64 = number_str
            .parse()
            .map_err(|_| LitCheckError::InvalidFormat {
                kind,
                msg: format!("Invalid number: {number_str}"),
            })?;
        int_val
    };

    Ok((value_nanos, unit, rest, found_decimal))
}
