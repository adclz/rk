#![allow(unused)]
use std::ops::Deref;

use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, File},
};

use crate::{
    hir::variable::{Literal, Numeric},
    ident::Ident,
};

pub trait ParseNumeric<'db> {
    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Numeric>;
}

impl<'db> ParseNumeric<'db> for ast::generated::BinaryInt_HexInt_OctalInt_SignedInt {
    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Numeric> {
        Ok(match self {
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::BinaryInt(binary_int) => {
                Numeric::Binary(Ident::from_node(db, file, binary_int)?)
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::HexInt(hex_int) => {
                Numeric::Hex(Ident::from_node(db, file, hex_int)?)
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::OctalInt(octal_int) => {
                Numeric::Octal(Ident::from_node(db, file, octal_int)?)
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::SignedInt(signed_int) => {
                Numeric::Signed(Ident::from_node(db, file, signed_int)?)
            }
        })
    }
}

pub(crate) trait ParseConstant<'db> {
    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Literal>;
}

impl<'db> ParseConstant<'db> for ast::generated::Constant {
    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Literal> {
        type Constant = ast::generated::BoolLiteral_CharLiteral_NumericLiteral_TimeLiteral;

        Ok(
            match self.children.deref() {
                Constant::BoolLiteral(bool_literal) => {
                    Literal::Bool(Ident::from_node(db, file, bool_literal.value.deref())?)
                }
                Constant::CharLiteral(char_literal) => match char_literal.char.children.deref() {
                    ast::generated::DByteCharStr_SByteCharStr::SByteCharStr(s_byte_char_str) => {
                        Literal::Char(Ident::from_node(db, file, s_byte_char_str.char.deref())?)
                    }
                    ast::generated::DByteCharStr_SByteCharStr::DByteCharStr(d_byte_char_str) => {
                        Literal::DChar(Ident::from_node(db, file, d_byte_char_str.char.deref())?)
                    }
                },
                Constant::NumericLiteral(numeric_literal) => {
                    match numeric_literal.children.deref() {
                    ast::generated::IntLiteral_RealLiteral::IntLiteral(int_literal) => {
                        match &int_literal.kind {
                            None => Literal::AnyNumeric(int_literal.int.parse(db, file)?),
                            Some(kind) => {
                                match kind.children.deref() {
                                    ast::generated::IntTypeName_MultibitsTypeName::IntTypeName(int_type_name) => {
                                        match int_type_name.children.deref() {
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(sign_int_type_name) => {
                                                match Ident::from_node(db, file, sign_int_type_name)?.text(db).as_str() {
                                                    "SINT" => Literal::SInt(int_literal.int.parse(db, file)?),
                                                    "INT" => Literal::Int(int_literal.int.parse(db, file)?),
                                                    "DINT" => Literal::DInt(int_literal.int.parse(db, file)?),
                                                    "LINT" => Literal::LInt(int_literal.int.parse(db, file)?),
                                                    _ => unreachable!(),
                                                }
                                            },
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(unsign_int_type_name) => {
                                                match  Ident::from_node(db, file, unsign_int_type_name)?.text(db).as_str() {
                                                    "USINT" => Literal::USInt(int_literal.int.parse(db, file)?),
                                                    "UINT" => Literal::UInt(int_literal.int.parse(db, file)?),
                                                    "UDINT" => Literal::UDInt(int_literal.int.parse(db, file)?),
                                                    "ULINT" => Literal::ULInt(int_literal.int.parse(db, file)?),
                                                    _ => unreachable!(),
                                                }
                                            },
                                        }
                                    },
                                    ast::generated::IntTypeName_MultibitsTypeName::MultibitsTypeName(multibits_type_name) => {
                                        match Ident::from_node(db, file, multibits_type_name)?.text(db).as_str() {
                                            "BYTE" => Literal::Byte(int_literal.int.parse(db, file)?),
                                            "WORD" => Literal::Word(int_literal.int.parse(db, file)?),
                                            "DWORD" => Literal::DWord(int_literal.int.parse(db, file)?),
                                            "LWORD" => Literal::LWord(int_literal.int.parse(db, file)?),
                                            _ => unreachable!(),
                                        }
                                    },
                                }
                            }
                        }
                    },
                    ast::generated::IntLiteral_RealLiteral::RealLiteral(real_literal) => {
                        match real_literal.children.deref() {
                            ast::generated::LReal_Real::Real(real) => {
                                Literal::Real(Ident::from_node(db, file, real.value.deref())?)
                            },
                            ast::generated::LReal_Real::LReal(l_real) => {
                                Literal::LReal(Ident::from_node(db, file, l_real.value.deref())?)
                            },
                        }
                    },
                }
                }
                Constant::TimeLiteral(time_literal) => match time_literal.children.deref() {
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::Date(date) => {
                        match date.children.deref() {
                            ast::generated::LongDate_ShortDate::LongDate(long_date) => {
                                Literal::LDate(Ident::from_node(db, file, long_date.value.deref())?)
                            }
                            ast::generated::LongDate_ShortDate::ShortDate(short_date) => {
                                Literal::Date(Ident::from_node(db, file, short_date.value.deref())?)
                            }
                        }
                    }
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::DateAndTime(
                        date_and_time,
                    ) => match date_and_time.children.deref() {
                        ast::generated::LongDateAndTime_ShortDateAndTime::LongDateAndTime(
                            long_date_and_time,
                        ) => Literal::LDateTime(Ident::from_node(
                            db,
                            file,
                            long_date_and_time.value.deref(),
                        )?),
                        ast::generated::LongDateAndTime_ShortDateAndTime::ShortDateAndTime(
                            short_date_and_time,
                        ) => Literal::DateTime(Ident::from_node(
                            db,
                            file,
                            short_date_and_time.value.deref(),
                        )?),
                    },
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::Duration(duration) => {
                        match duration.children.deref() {
                            ast::generated::Ltime_Time::Ltime(ltime) => {
                                Literal::LTime(Ident::from_node(db, file, ltime.value.deref())?)
                            }
                            ast::generated::Ltime_Time::Time(time) => {
                                Literal::Time(Ident::from_node(db, file, time.value.deref())?)
                            }
                        }
                    }
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::TimeOfDay(time_of_day) => {
                        match time_of_day.children.deref() {
                            ast::generated::Ltod_Tod::Ltod(ltod) => {
                                Literal::LTod(Ident::from_node(db, file, ltod.value.deref())?)
                            }
                            ast::generated::Ltod_Tod::Tod(tod) => {
                                Literal::Tod(Ident::from_node(db, file, tod.value.deref())?)
                            }
                        }
                    }
                },
            },
        )
    }
}
