#![allow(unused)]
use std::ops::Deref;

use auto_lsp::{
    anyhow,
    core::ast::AstNode,
    default::db::{BaseDatabase, File},
};

use crate::{
    hir::expression::{Expr, Literal, Numeric},
    ident::Ident,
    solver::NamespacePath,
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
    fn to_constant(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>>;
}

impl<'db> ParseConstant<'db> for ast::generated::Expression {
    fn to_constant(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        match self {
            ast::generated::Expression::PrimaryExpression(p) => match p {
                ast::generated::PrimaryExpression::Constant(c) => c.to_constant(db, file),
                ast::generated::PrimaryExpression::FqName(path) => {
                    Ok(Expr::new_target(
                        db,
                        *path.get_range(),
                        NamespacePath::from((
                            db,
                             path
                                .fragment
                                .iter()
                                .map(|f| Ident::from_node(db, file, f.deref()))
                                .collect::<anyhow::Result<Vec<_>>>()?,
                        )),
                        Ident::from_node(db, file, path.target.deref())?,
                    ))
                },
                ast::generated::PrimaryExpression::EnumValue(enum_value) => {
                    Ok(Expr::new_enum_value(db, *enum_value.get_range(), Ident::from_node(db, file, enum_value.children.deref())?))
                },
                _ => unreachable!(),
            },
            _ => todo!(),
        }
    }
}

impl<'db> ParseConstant<'db> for ast::generated::Constant {
    fn to_constant(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        type Constant = ast::generated::BoolLiteral_CharLiteral_NumericLiteral_TimeLiteral;
        Ok(Expr::new_literal(
            db,
            *self.children.get_range(),
            match self.children.deref() {
                Constant::BoolLiteral(bool_literal) => {
                    Literal::Bool(Ident::from_node(db, file, bool_literal.value.deref())?)
                }
                Constant::CharLiteral(char_literal) => todo!(),
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
                                                match sign_int_type_name.children.deref() {
                                                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Literal::SInt(int_literal.int.parse(db, file)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Literal::Int(int_literal.int.parse(db, file)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Literal::DInt(int_literal.int.parse(db, file)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Literal::LInt(int_literal.int.parse(db, file)?),
                                                }
                                            },
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(unsign_int_type_name) => {
                                                match unsign_int_type_name.children.deref() {
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => Literal::USInt(int_literal.int.parse(db, file)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => Literal::UInt(int_literal.int.parse(db, file)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => Literal::UDInt(int_literal.int.parse(db, file)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => Literal::ULInt(int_literal.int.parse(db, file)?),
                                                }
                                            },
                                        }
                                    },
                                    ast::generated::IntTypeName_MultibitsTypeName::MultibitsTypeName(multibits_type_name) => {
                                        match multibits_type_name.children.deref() {
                                            ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => Literal::Byte(int_literal.int.parse(db, file)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => Literal::Word(int_literal.int.parse(db, file)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => Literal::DWord(int_literal.int.parse(db, file)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => Literal::LWord(int_literal.int.parse(db, file)?),
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
        ))
    }
}
