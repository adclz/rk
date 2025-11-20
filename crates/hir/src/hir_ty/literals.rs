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
};

use std::{num::ParseIntError, u8};

use time::{Date, Duration, PrimitiveDateTime, Time, macros::format_description};
/* 
impl<'db> ElementarySpec {
    pub fn check_literal(
        &self,
        db: &'db dyn BaseDatabase,
        spec: Elementary,
    ) -> Result<(), InferLiteralError> {
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
                Elementary::InferIdent(ident) => ident
                    .as_date(db)
                    .map(|_| ())
                    .map_err(|e| InferLiteralError::Invalid_DATE_Format(e.to_string())),
                _ => Err(InferLiteralError::Invalid_DATE_Literal),
            },
            ElementarySpec::LDate => match spec {
                Elementary::InferIdent(ident) => ident
                    .as_long_date(db)
                    .map(|_| ())
                    .map_err(|e| InferLiteralError::Invalid_LDATE_Format(e.to_string())),
                _ => Err(InferLiteralError::Invalid_LDATE_Literal),
            },
            ElementarySpec::Tod => match spec {
                Elementary::InferIdent(ident) | Elementary::TimeOfDay(ident) => ident
                    .as_tod(db)
                    .map(|_| ())
                    .map_err(|e| InferLiteralError::Invalid_TOD_Format(e.to_string())),
                _ => Err(InferLiteralError::Invalid_TOD_Literal),
            },
            ElementarySpec::LTod => match spec {
                Elementary::InferIdent(ident) | Elementary::LTod(ident) => ident
                    .as_long_tod(db)
                    .map(|_| ())
                    .map_err(|e| InferLiteralError::Invalid_LTOD_Format(e.to_string())),
                _ => Err(InferLiteralError::Invalid_LTOD_Literal),
            },
            ElementarySpec::Dt => match spec {
                Elementary::InferIdent(ident) => ident
                    .as_date_time(db)
                    .map_err(|e| InferLiteralError::Invalid_DT_Format(e.to_string()))
                    .map(|_| ()),
                _ => Err(InferLiteralError::Invalid_DT_Literal),
            },

            // 6a/b LDT / LDATE_AND_TIME
            ElementarySpec::Ldt => match spec {
                Elementary::InferIdent(ident) => ident
                    .as_long_date_time(db)
                    .map_err(|e| InferLiteralError::Invalid_LDT_Format(e.to_string()))
                    .map(|_| ()),
                _ => Err(InferLiteralError::Invalid_LDT_Literal),
            },
            ElementarySpec::Time => match spec {
                Elementary::InferIdent(ident) | Elementary::Time(ident) => {
                    ident.as_time(db).map(|_| ())
                }
                _ => Err(InferLiteralError::Invalid_TIME_Literal),
            },
            ElementarySpec::LTime => match spec {
                Elementary::InferIdent(ident) | Elementary::LTime(ident) => {
                    ident.as_ltime(db).map(|_| ())
                }
                _ => Err(InferLiteralError::Invalid_LTIME_Literal),
            },

            ElementarySpec::String => match spec {
                Elementary::InferIdent(ident) => ident.as_single_string(db).map(|_| ()),
                _ => Err(InferLiteralError::Invalid_STRING_Literal),
            },
            ElementarySpec::WString => match spec {
                Elementary::InferIdent(ident) => ident.as_double_string(db).map(|_| ()),
                _ => Err(InferLiteralError::Invalid_WSTRING_Literal),
            },
            ElementarySpec::Char => todo!(),
            ElementarySpec::WChar => todo!(),
        }
    }
}
*/