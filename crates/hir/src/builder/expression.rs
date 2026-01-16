use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::types::ParseMultiBits;
use crate::check::errors::analysis_error::AnalysisError;
use crate::check::errors::syntax::SyntaxError;
use crate::hir_def::expressions::expression::{
    BeginPathExpr, FieldExpr, FuncCall, IndexExpr, Integer, IntegerKind, ParamAssignKind, PathExpr,
    VariableAccessKind,
};
use crate::hir_def::expressions::invocation::{Invocation, InvocationKind};
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::variable::DirectVariable;
use crate::{
    hir_def::expressions::expression::{
        AddOperatorKind, BooleanOperatorKind, ComparisonOperatorKind, Elementary, Expr, ExprKind,
        MultOperatorKind, ParamAssign, PathExprKind, PrimaryExpr, RefValue, UnaryOperatorKind,
        VarAccess, VariableAccess,
    },
    hir_def::interned::identifier::Ident,
};
pub trait ParseExpression<'db> {
    fn to_expr(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Expr<'db>, AnalysisError<'db>>;
}

impl<'db> ParseExpression<'db> for ast::generated::Expression {
    fn to_expr(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Expr<'db>, AnalysisError<'db>> {
        match self {
            ast::generated::Expression::PrimaryExpression(p) => p.to_expr(sema),
            ast::generated::Expression::BooleanOperator(boolean_operator) => match boolean_operator
                .children
                .cast(sema.ast)
            {
                ast::generated::AndOperator_OrOperator_XorOperator::OrOperator(or_operator) => {
                    let left = or_operator.left.cast(sema.ast).to_expr(sema)?;
                    let right = or_operator.right.cast(sema.ast).to_expr(sema)?;

                    Ok(Expr::new(
                        sema.db,
                        ExprKind::BooleanOperator {
                            left,
                            operator: BooleanOperatorKind::Or,
                            right,
                        },
                        or_operator.into(),
                        sema.current_scope,
                    ))
                }
                ast::generated::AndOperator_OrOperator_XorOperator::XorOperator(xor_operator) => {
                    let left = xor_operator.left.cast(sema.ast).to_expr(sema)?;
                    let right = xor_operator.right.cast(sema.ast).to_expr(sema)?;

                    Ok(Expr::new(
                        sema.db,
                        ExprKind::BooleanOperator {
                            left,
                            operator: BooleanOperatorKind::Xor,
                            right,
                        },
                        xor_operator.into(),
                        sema.current_scope,
                    ))
                }
                ast::generated::AndOperator_OrOperator_XorOperator::AndOperator(and_operator) => {
                    let left = and_operator.left.cast(sema.ast).to_expr(sema)?;
                    let right = and_operator.right.cast(sema.ast).to_expr(sema)?;

                    Ok(Expr::new(
                        sema.db,
                        ExprKind::BooleanOperator {
                            left,
                            operator: BooleanOperatorKind::And,
                            right,
                        },
                        and_operator.into(),
                        sema.current_scope,
                    ))
                }
            },
            ast::generated::Expression::ComparisonOperator(comparison_operator) => {
                match comparison_operator.children.cast(sema.ast) {
                    ast::generated::EqOperator_OrdOperator::EqOperator(eq_operator) => {
                        let left = eq_operator.left.cast(sema.ast).to_expr(sema)?;
                        let right = eq_operator.right.cast(sema.ast).to_expr(sema)?;

                        let operator = match eq_operator.operator.cast(sema.ast) {
                            ast::generated::Eq::Token_Equal(_) => ComparisonOperatorKind::Eq,
                            ast::generated::Eq::Token_LessGreater(_) => ComparisonOperatorKind::Ne,
                        };

                        Ok(Expr::new(
                            sema.db,
                            ExprKind::ComparisonOperator {
                                left,
                                operator,
                                right,
                            },
                            eq_operator.into(),
                            sema.current_scope,
                        ))
                    }
                    ast::generated::EqOperator_OrdOperator::OrdOperator(ord_operator) => {
                        let left = ord_operator.left.cast(sema.ast).to_expr(sema)?;
                        let right = ord_operator.right.cast(sema.ast).to_expr(sema)?;

                        let operator = match ord_operator.operator.cast(sema.ast) {
                            ast::generated::Ord::Token_Less(_) => ComparisonOperatorKind::Lt,
                            ast::generated::Ord::Token_Greater(_) => ComparisonOperatorKind::Gt,
                            ast::generated::Ord::Token_LessEqual(_) => ComparisonOperatorKind::Le,
                            ast::generated::Ord::Token_GreaterEqual(_) => {
                                ComparisonOperatorKind::Ge
                            }
                        };

                        Ok(Expr::new(
                            sema.db,
                            ExprKind::ComparisonOperator {
                                left,
                                operator,
                                right,
                            },
                            ord_operator.into(),
                            sema.current_scope,
                        ))
                    }
                }
            }
            ast::generated::Expression::AddOperator(add_operator) => {
                let left = add_operator.left.cast(sema.ast).to_expr(sema)?;
                let right = add_operator.right.cast(sema.ast).to_expr(sema)?;
                let operator = match add_operator.operator.cast(sema.ast) {
                    ast::generated::Add::Token_Plus(_) => AddOperatorKind::Plus,
                    ast::generated::Add::Token_Minus(_) => AddOperatorKind::Minus,
                };

                Ok(Expr::new(
                    sema.db,
                    ExprKind::AddOperator {
                        left,
                        operator,
                        right,
                    },
                    add_operator.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::Expression::MultOperator(mult_operator) => {
                let left = mult_operator.left.cast(sema.ast).to_expr(sema)?;
                let right = mult_operator.right.cast(sema.ast).to_expr(sema)?;
                let operator = match mult_operator.operator.cast(sema.ast) {
                    ast::generated::Mult::Token_Star(_) => MultOperatorKind::Mul,
                    ast::generated::Mult::Token_Slash(_) => MultOperatorKind::Div,
                    ast::generated::Mult::Token_MOD(_) => MultOperatorKind::Mod,
                };

                Ok(Expr::new(
                    sema.db,
                    ExprKind::MultOperator {
                        left,
                        operator,
                        right,
                    },
                    mult_operator.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::Expression::PowerOperator(power_operator) => {
                let left = power_operator.left.cast(sema.ast).to_expr(sema)?;
                let right = power_operator.right.cast(sema.ast).to_expr(sema)?;

                Ok(Expr::new(
                    sema.db,
                    ExprKind::PowerOperator { left, right },
                    power_operator.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::Expression::UnaryOperator(unary_operator) => {
                let expr = unary_operator.expr.cast(sema.ast).to_expr(sema)?;

                let operator = match unary_operator.operator.cast(sema.ast) {
                    ast::generated::Unary::Token_Plus(_) => UnaryOperatorKind::Plus,
                    ast::generated::Unary::Token_Minus(_) => UnaryOperatorKind::Minus,
                    ast::generated::Unary::Token_NOT(_) => UnaryOperatorKind::Not,
                };

                Ok(Expr::new(
                    sema.db,
                    ExprKind::UnaryOperator { expr, operator },
                    unary_operator.into(),
                    sema.current_scope,
                ))
            }
        }
    }
}

impl<'db> ParseExpression<'db> for ast::generated::PrimaryExpression {
    fn to_expr(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Expr<'db>, AnalysisError<'db>> {
        match self {
            ast::generated::PrimaryExpression::EnumValue(enum_) => {
                let name = enum_.enum_path.cast(sema.ast).parse(sema)?;
                let variant = SpanIdent::from_node(sema.db, sema, enum_.children.cast(sema.ast))?;

                Ok(Expr::new(
                    sema.db,
                    ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { name, variant }),
                    enum_.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::PrimaryExpression::Constant(c) => c.to_expr(sema),
            ast::generated::PrimaryExpression::VariableAccess(v) => {
                let variable = v.to_access(sema)?;
                Ok(Expr::new(
                    sema.db,
                    ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(v.to_access(sema)?)),
                    v.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::PrimaryExpression::FuncCall(func) => {
                let target = func.function.cast(sema.ast).parse(sema)?;

                let mut parameters = vec![];
                for params in func.params.iter() {
                    match params.cast(sema.ast) {
                        ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                        ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                            match p.children.cast(sema.ast) {
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                                        parameters.push(ParamAssign::new(sema.db, p.into(), sema.current_scope, match p.param.as_ref() {
                                            Some(param) => {
                                                ParamAssignKind::FormalInput { param: SpanIdent::from_node(sema.db, sema, param.cast(sema.ast))?, value: p.value.cast(sema.ast).to_expr(sema)? }
                                            },
                                            None => {
                                                ParamAssignKind::NonFormal { value: p.value.cast(sema.ast).to_expr(sema)? }
                                            }
                                        }))
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let variable = p.variable.cast(sema.ast).to_access(sema)?;

                                        parameters.push(ParamAssign::new(sema.db, p.into(), sema.current_scope,
                                            ParamAssignKind::FormalOutput { not: p.not.is_some() , param: SpanIdent::from_node(sema.db, sema, p.param.cast(sema.ast))?, variable })
                                    )
                                    }
                                }
                        }
                    }
                }
                Ok(Expr::new(
                    sema.db,
                    ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(FuncCall::new(
                        sema.db, target, parameters,
                    ))),
                    func.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::PrimaryExpression::ParenthesizedExpression(p) => Ok(Expr::new(
                sema.db,
                ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr {
                    expr: p.children.cast(sema.ast).to_expr(sema)?,
                }),
                p.into(),
                sema.current_scope,
            )),
            ast::generated::PrimaryExpression::RefValue(r) => match r.children.cast(sema.ast) {
                ast::generated::Null_RefAddr::Null(_) => Ok(Expr::new(
                    sema.db,
                    ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                        value: RefValue::Null,
                    }),
                    r.into(),
                    sema.current_scope,
                )),
                ast::generated::Null_RefAddr::RefAddr(a) => Ok(Expr::new(
                    sema.db,
                    ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                        value: RefValue::Address(a.children.cast(sema.ast).parse(sema)?),
                    }),
                    a.into(),
                    sema.current_scope,
                )),
            },
        }
    }
}

pub trait ParseNumeric<'db> {
    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Integer, AnalysisError<'db>>;
}

impl<'db> ParseNumeric<'db> for ast::generated::BinaryInt_HexInt_OctalInt_SignedInt {
    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Integer, AnalysisError<'db>> {
        Ok(match self {
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::BinaryInt(binary_int) => {
                Integer::new(
                    sema.db,
                    Ident::from_node(sema.db, sema.file, binary_int)?,
                    IntegerKind::Binary,
                )
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::HexInt(hex_int) => Integer::new(
                sema.db,
                Ident::from_node(sema.db, sema.file, hex_int)?,
                IntegerKind::Hex,
            ),
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::OctalInt(octal_int) => {
                Integer::new(
                    sema.db,
                    Ident::from_node(sema.db, sema.file, octal_int)?,
                    IntegerKind::Octal,
                )
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::SignedInt(signed_int) => {
                Integer::new(
                    sema.db,
                    Ident::from_node(sema.db, sema.file, signed_int)?,
                    IntegerKind::Signed,
                )
            }
        })
    }
}

impl<'db> ParseExpression<'db> for ast::generated::Constant {
    fn to_expr(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Expr<'db>, AnalysisError<'db>> {
        type Constant = ast::generated::BoolLiteral_CharLiteral_NumericLiteral_TimeLiteral;

        let lit = match self.children.cast(sema.ast) {
                Constant::BoolLiteral(bool_literal) => {
                    match bool_literal.children.cast(sema.ast) {
                        ast::generated::BoolLiteralWithNumeric_BoolLiteralWithString::BoolLiteralWithNumeric(bool_literal) => {
                            Elementary::Bool(Ident::from_node(sema.db, sema.file, bool_literal.value.cast(sema.ast))?)
                        }
                        ast::generated::BoolLiteralWithNumeric_BoolLiteralWithString::BoolLiteralWithString(bool_literal) => {
                           Elementary::Bool(Ident::from_node(sema.db, sema.file, bool_literal.value.cast(sema.ast))?)
                        }
                    }
                }
                Constant::CharLiteral(char_literal) => {
                    Elementary::AnyString(Ident::from_node(sema.db, sema.file, char_literal.value.cast(sema.ast))?)
                }
                Constant::NumericLiteral(numeric_literal) => {
                    match numeric_literal.children.cast(sema.ast) {
                    ast::generated::IntLiteral_RealLiteral::IntLiteral(int_literal) => {
                        match &int_literal.kind {
                            None => Elementary::InferInteger(int_literal.int.cast(sema.ast).parse(sema)?),
                            Some(kind) => {
                                match kind.cast(sema.ast).children.cast(sema.ast) {
                                    ast::generated::IntTypeName_MultibitsTypeName::IntTypeName(int_type_name) => {
                                        match int_type_name.children.cast(sema.ast) {
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(sign_int_type_name) => {
                                                match sign_int_type_name.children.cast(sema.ast) {
                                                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Elementary::SInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Elementary::Int(int_literal.int.cast(sema.ast).parse(sema)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Elementary::DInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Elementary::LInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                }
                                            },
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(unsign_int_type_name) => {
                                                match unsign_int_type_name.children.cast(sema.ast) {
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => Elementary::USInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => Elementary::UInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => Elementary::UDInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => Elementary::ULInt(int_literal.int.cast(sema.ast).parse(sema)?),
                                                }
                                            },
                                        }
                                    },
                                    ast::generated::IntTypeName_MultibitsTypeName::MultibitsTypeName(multibits_type_name) => {
                                        match multibits_type_name.children.cast(sema.ast) {
                                            ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => Elementary::Byte(int_literal.int.cast(sema.ast).parse(sema)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => Elementary::Word(int_literal.int.cast(sema.ast).parse(sema)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => Elementary::DWord(int_literal.int.cast(sema.ast).parse(sema)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => Elementary::LWord(int_literal.int.cast(sema.ast).parse(sema)?),
                                        }
                                    },
                                }
                            }
                        }
                    },
                    ast::generated::IntLiteral_RealLiteral::RealLiteral(real_literal) => {
                        match real_literal.Type.as_ref().map(|t| t.cast(sema.ast)) {
                            Some(kind) => {
                                match kind.children.cast(sema.ast) {
                                    ast::generated::LrealName_RealName::RealName(_) => {
                                        Elementary::Real(Ident::from_node(sema.db, sema.file, real_literal.value.cast(sema.ast))?)
                                    }
                                    ast::generated::LrealName_RealName::LrealName(_) => {
                                        Elementary::LReal(Ident::from_node(sema.db, sema.file, real_literal.value.cast(sema.ast))?)
                                    }
                                }
                            }
                            None => Elementary::InferFloat(Ident::from_node(sema.db, sema.file, real_literal.value.cast(sema.ast))?)
                        }
                    },
                }
                }
                Constant::TimeLiteral(time_literal) => match time_literal.children.cast(sema.ast) {
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::Date(date) => {
                        match date.children.cast(sema.ast) {
                            ast::generated::LongDate_ShortDate::LongDate(date) => {
                                Elementary::LDate(Ident::from_node(sema.db, sema.file, date.value.cast(sema.ast))?)
                            }
                            ast::generated::LongDate_ShortDate::ShortDate(date) => {
                                Elementary::Date(Ident::from_node(sema.db, sema.file, date.value.cast(sema.ast))?)
                            }
                        }
                    }
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::DateAndTime(
                        date_and_time,
                    ) => match date_and_time.children.cast(sema.ast) {
                        ast::generated::LongDateAndTime_ShortDateAndTime::LongDateAndTime(
                            dt,
                        ) => Elementary::LDateTime(Ident::from_node(sema.db, sema.file, dt.value.cast(sema.ast))?),
                        ast::generated::LongDateAndTime_ShortDateAndTime::ShortDateAndTime(
                            dt,
                        ) => Elementary::DateAndTime(Ident::from_node(sema.db, sema.file, dt.value.cast(sema.ast))?),
                    },
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::Duration(duration) => {
                        match duration.children.cast(sema.ast) {
                            ast::generated::Ltime_Time::Ltime(ltime) => {
                                Elementary::LTime(Ident::from_node(sema.db, sema.file, ltime.value.cast(sema.ast))?)
                            }
                            ast::generated::Ltime_Time::Time(time) => {
                                Elementary::Time(Ident::from_node(sema.db, sema.file, time.value.cast(sema.ast))?)
                            }
                        }
                    }
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::TimeOfDay(time_of_day) => {
                        match time_of_day.children.cast(sema.ast) {
                            ast::generated::Ltod_Tod::Ltod(ltod) => {
                                Elementary::LTod(Ident::from_node(sema.db, sema.file, ltod.value.cast(sema.ast))?)
                            }
                            ast::generated::Ltod_Tod::Tod(tod) => {
                                Elementary::TimeOfDay(Ident::from_node(sema.db, sema.file, tod.value.cast(sema.ast))?)
                            }
                        }
                    }
                },
            };

        Ok(Expr::new(
            sema.db,
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)),
            self.into(),
            sema.current_scope,
        ))
    }
}

pub trait ParseDirectVariable<'db> {
    fn to_direct_variable(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<DirectVariable, AnalysisError<'db>>;
}

impl<'db> ParseDirectVariable<'db> for ast::generated::DirectVariable {
    fn to_direct_variable(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<DirectVariable, AnalysisError<'db>> {
        let adress = Ident::from_node(sema.db, sema.file, self.adress.cast(sema.ast))?;

        let (offset, partly) = match self.offset.cast(sema.ast) {
            ast::generated::Offset_Partly::Offset(offset) => {
                (Some(Ident::from_node(sema.db, sema.file, offset)?), false)
            }
            ast::generated::Offset_Partly::Partly(partly) => (None, true),
        };

        Ok(DirectVariable {
            adress,
            partly,
            offset,
        })
    }
}

pub trait ParseVariableAccess<'db> {
    fn to_access(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<VariableAccess<'db>, AnalysisError<'db>>;
}

impl<'db> ParseVariableAccess<'db> for ast::generated::VariableAccess {
    fn to_access(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<VariableAccess<'db>, AnalysisError<'db>> {
        let multibits = self.access.as_ref().and_then(|multibits| {
            match multibits.cast(sema.ast).to_multibits(sema) {
                Ok(mb) => Some(mb),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            }
        });

        match self.variable.cast(sema.ast).children.cast(sema.ast) {
            ast::generated::BeginPathExpression_DirectVariable::DirectVariable(v) => {
                Ok(VariableAccess::new(
                    sema.db,
                    VariableAccessKind::Direct(v.to_direct_variable(sema)?),
                    multibits,
                    self.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::BeginPathExpression_DirectVariable::BeginPathExpression(v) => {
                Ok(VariableAccess::new(
                    sema.db,
                    VariableAccessKind::Symbolic(v.parse(sema)?),
                    multibits,
                    self.into(),
                    sema.current_scope,
                ))
            }
        }
    }
}

impl<'db> ParseVariableAccess<'db> for ast::generated::Variable {
    fn to_access(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<VariableAccess<'db>, AnalysisError<'db>> {
        match self.children.cast(sema.ast) {
            ast::generated::BeginPathExpression_DirectVariable::DirectVariable(v) => {
                let adress = Ident::from_node(sema.db, sema.file, v.adress.cast(sema.ast))?;

                let (offset, partly) = match v.offset.cast(sema.ast) {
                    ast::generated::Offset_Partly::Offset(offset) => {
                        (Some(Ident::from_node(sema.db, sema.file, v)?), false)
                    }
                    ast::generated::Offset_Partly::Partly(partly) => (None, true),
                };

                Ok(VariableAccess::new(
                    sema.db,
                    VariableAccessKind::Direct(v.to_direct_variable(sema)?),
                    None,
                    self.into(),
                    sema.current_scope,
                ))
            }
            ast::generated::BeginPathExpression_DirectVariable::BeginPathExpression(v) => {
                Ok(VariableAccess::new(
                    sema.db,
                    VariableAccessKind::Symbolic(v.parse(sema)?),
                    None,
                    self.into(),
                    sema.current_scope,
                ))
            }
        }
    }
}

impl<'db> ParseVariableAccess<'db> for ast::generated::DirectVariable {
    fn to_access(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<VariableAccess<'db>, AnalysisError<'db>> {
        Ok(VariableAccess::new(
            sema.db,
            VariableAccessKind::Direct(self.to_direct_variable(sema)?),
            None,
            self.into(),
            sema.current_scope,
        ))
    }
}

pub trait ParseExpr<'db> {
    type Output;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>>;
}

impl<'db> ParseExpr<'db> for ast::generated::BeginPathExpression {
    type Output = BeginPathExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        match self.children.cast(sema.ast) {
            ast::generated::Invocation_PathExpression::Invocation(i) => {
                match i.children.cast(sema.ast) {
                    ast::generated::AnyInvocation_SuperBodyInvocation::SuperBodyInvocation(s) => {
                        let invocation = Invocation::new(
                            sema.db,
                            s.into(),
                            s.SUPER.cast(sema.ast).into(),
                            sema.current_scope,
                            InvocationKind::SuperBody,
                        );

                        Ok(BeginPathExpr::new(
                            sema.db,
                            Some(invocation),
                            None,
                            self.into(),
                            sema.current_scope,
                        ))
                    }
                    ast::generated::AnyInvocation_SuperBodyInvocation::AnyInvocation(any) => {
                        let path = any
                            .children
                            .as_ref()
                            .and_then(|s| {
                                s.cast(sema.ast)
                                    .path
                                    .as_ref()
                                    .map(|p| p.cast(sema.ast).parse(sema))
                            })
                            .transpose()?;

                        let invocation = match any.invocation.cast(sema.ast) {
                            ast::generated::SuperInvocation_ThisInvocation::SuperInvocation(s) => {
                                Invocation::new(
                                    sema.db,
                                    self.into(),
                                    s.SUPER.cast(sema.ast).into(),
                                    sema.current_scope,
                                    InvocationKind::Super,
                                )
                            }
                            ast::generated::SuperInvocation_ThisInvocation::ThisInvocation(t) => {
                                Invocation::new(
                                    sema.db,
                                    self.into(),
                                    t.THIS.cast(sema.ast).into(),
                                    sema.current_scope,
                                    InvocationKind::This,
                                )
                            }
                        };

                        Ok(BeginPathExpr::new(
                            sema.db,
                            Some(invocation),
                            path,
                            self.into(),
                            sema.current_scope,
                        ))
                    }
                }
            }
            ast::generated::Invocation_PathExpression::PathExpression(p) => Ok(BeginPathExpr::new(
                sema.db,
                None,
                Some(p.parse(sema)?),
                self.into(),
                sema.current_scope,
            )),
        }
    }
}

impl<'db> ParseExpr<'db> for ast::generated::PathExpression {
    type Output = PathExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        Ok(match self.children.cast(sema.ast) {
            ast::generated::FieldExpression_IndexExpression_VarAccess::FieldExpression(
                field_expr,
            ) => PathExpr::new(
                sema.db,
                PathExprKind::Field(field_expr.parse(sema)?),
                self.into(),
                sema.current_scope,
            ),
            ast::generated::FieldExpression_IndexExpression_VarAccess::IndexExpression(
                index_expr,
            ) => PathExpr::new(
                sema.db,
                PathExprKind::Index(index_expr.parse(sema)?),
                self.into(),
                sema.current_scope,
            ),
            ast::generated::FieldExpression_IndexExpression_VarAccess::VarAccess(var_access) => {
                PathExpr::new(
                    sema.db,
                    PathExprKind::VarAccess(var_access.parse(sema)?),
                    self.into(),
                    sema.current_scope,
                )
            }
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::FieldExpression {
    type Output = FieldExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        Ok(FieldExpr {
            path: self.path.cast(sema.ast).parse(sema)?,
            var: self.target.cast(sema.ast).parse(sema)?,
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::IndexExpression {
    type Output = IndexExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        Ok(IndexExpr {
            path: self.children.cast(sema.ast).parse(sema)?,
            index: self
                .index
                .cast(sema.ast)
                .children
                .iter()
                .map(|i| i.cast(sema.ast).children.cast(sema.ast).to_expr(sema))
                .collect::<Result<Vec<_>, AnalysisError<'db>>>()?,
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::VarAccess {
    type Output = VarAccess<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        match self.children.cast(sema.ast) {
            ast::generated::ERRUnexpectedSuperInPath_ERRUnexpectedThisInPath_Field_RefDeref::ERRUnexpectedThisInPath(
                direct_variable,
            ) => Err(AnalysisError::SyntaxError(SyntaxError::UnexpectedThis(
                direct_variable.get_span(),
            ))),
            ast::generated::ERRUnexpectedSuperInPath_ERRUnexpectedThisInPath_Field_RefDeref::ERRUnexpectedSuperInPath(
                direct_variable,
            ) => Err(AnalysisError::SyntaxError(SyntaxError::UnexpectedSuper(
                direct_variable.get_span(),
            ))),
            ast::generated::ERRUnexpectedSuperInPath_ERRUnexpectedThisInPath_Field_RefDeref::Field(field) => Ok(
                VarAccess::Simple(SpanIdent::from_node(sema.db, sema, field)?),
            ),
            ast::generated::ERRUnexpectedSuperInPath_ERRUnexpectedThisInPath_Field_RefDeref::RefDeref(ref_deref) => {
                Ok(VarAccess::Deref(SpanIdent::from_node(
                    sema.db,
                    sema,
                    ref_deref.Ref.cast(sema.ast),
                )?, ref_deref.children.len() as u16))
            }
        }
    }
}
