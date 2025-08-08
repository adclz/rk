use std::ops::Deref;

use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use crate::check::errors::semantic_errors::{invocation_in_expression, unexpected_this};
use crate::hir::expressions::expression::{
    FieldExpr, IndexExpr, Numeric, NumericKind, PathExpr, VariableAccessKind,
};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::{
    hir::expressions::expression::{
        AddOperatorKind, BooleanOperatorKind, ComparisonOperatorKind, Elementary, Expr, ExprKind,
        MultOperatorKind, ParamAssign, PathExprKind, PrimaryExpr, RefAdress, RefValue,
        SymbolicVariable, UnaryOperatorKind, VarAccess, VariableAccess,
    },
    hir::interned::identifier::Ident,
};
pub trait ParseExpression<'db> {
    fn to_expr(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>>;
}

impl<'db> ParseExpression<'db> for ast::generated::Expression {
    fn to_expr(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        match self {
            ast::generated::Expression::PrimaryExpression(p) => p.to_expr(sema),
            ast::generated::Expression::BooleanOperator(boolean_operator) => match boolean_operator
                .children
                .deref()
            {
                ast::generated::AndOperator_OrOperator_XorOperator::OrOperator(or_operator) => {
                    let left = or_operator.left.to_expr(sema)?;
                    let right = or_operator.right.to_expr(sema)?;

                    Ok(Expr::new(
                        sema.db,
                        or_operator.get_span(),
                        ExprKind::BooleanOperator {
                            left,
                            operator: BooleanOperatorKind::Or,
                            right,
                        },
                        sema.current_scope,
                    ))
                }
                ast::generated::AndOperator_OrOperator_XorOperator::XorOperator(xor_operator) => {
                    let left = xor_operator.left.to_expr(sema)?;
                    let right = xor_operator.right.to_expr(sema)?;

                    Ok(Expr::new(
                        sema.db,
                        xor_operator.get_span(),
                        ExprKind::BooleanOperator {
                            left,
                            operator: BooleanOperatorKind::Xor,
                            right,
                        },
                        sema.current_scope,
                    ))
                }
                ast::generated::AndOperator_OrOperator_XorOperator::AndOperator(and_operator) => {
                    let left = and_operator.left.to_expr(sema)?;
                    let right = and_operator.right.to_expr(sema)?;

                    Ok(Expr::new(
                        sema.db,
                        and_operator.get_span(),
                        ExprKind::BooleanOperator {
                            left,
                            operator: BooleanOperatorKind::And,
                            right,
                        },
                        sema.current_scope,
                    ))
                }
            },
            ast::generated::Expression::ComparisonOperator(comparison_operator) => {
                match comparison_operator.children.deref() {
                    ast::generated::EqOperator_OrdOperator::EqOperator(eq_operator) => {
                        let left = eq_operator.left.to_expr(sema)?;
                        let right = eq_operator.right.to_expr(sema)?;

                        let operator = match eq_operator.operator.deref() {
                            ast::generated::Eq::Token_Equal(_) => ComparisonOperatorKind::Eq,
                            ast::generated::Eq::Token_LessGreater(_) => ComparisonOperatorKind::Ne,
                        };

                        Ok(Expr::new(
                            sema.db,
                            eq_operator.get_span(),
                            ExprKind::ComparisonOperator {
                                left,
                                operator,
                                right,
                            },
                            sema.current_scope,
                        ))
                    }
                    ast::generated::EqOperator_OrdOperator::OrdOperator(ord_operator) => {
                        let left = ord_operator.left.to_expr(sema)?;
                        let right = ord_operator.right.to_expr(sema)?;

                        let operator = match ord_operator.operator.deref() {
                            ast::generated::Ord::Token_Less(_) => ComparisonOperatorKind::Lt,
                            ast::generated::Ord::Token_Greater(_) => ComparisonOperatorKind::Gt,
                            ast::generated::Ord::Token_LessEqual(_) => ComparisonOperatorKind::Le,
                            ast::generated::Ord::Token_GreaterEqual(_) => {
                                ComparisonOperatorKind::Ge
                            }
                        };

                        Ok(Expr::new(
                            sema.db,
                            ord_operator.get_span(),
                            ExprKind::ComparisonOperator {
                                left,
                                operator,
                                right,
                            },
                            sema.current_scope,
                        ))
                    }
                }
            }
            ast::generated::Expression::AddOperator(add_operator) => {
                let left = add_operator.left.to_expr(sema)?;
                let right = add_operator.right.to_expr(sema)?;
                let operator = match add_operator.operator.deref() {
                    ast::generated::Add::Token_Plus(_) => AddOperatorKind::Plus,
                    ast::generated::Add::Token_Minus(_) => AddOperatorKind::Minus,
                };

                Ok(Expr::new(
                    sema.db,
                    add_operator.get_span(),
                    ExprKind::AddOperator {
                        left,
                        operator,
                        right,
                    },
                    sema.current_scope,
                ))
            }
            ast::generated::Expression::MultOperator(mult_operator) => {
                let left = mult_operator.left.to_expr(sema)?;
                let right = mult_operator.right.to_expr(sema)?;
                let operator = match mult_operator.operator.deref() {
                    ast::generated::Mult::Token_Star(_) => MultOperatorKind::Mul,
                    ast::generated::Mult::Token_Slash(_) => MultOperatorKind::Div,
                    ast::generated::Mult::Token_MOD(_) => MultOperatorKind::Mod,
                };

                Ok(Expr::new(
                    sema.db,
                    mult_operator.get_span(),
                    ExprKind::MultOperator {
                        left,
                        operator,
                        right,
                    },
                    sema.current_scope,
                ))
            }
            ast::generated::Expression::PowerOperator(power_operator) => {
                let left = power_operator.left.to_expr(sema)?;
                let right = power_operator.right.to_expr(sema)?;

                Ok(Expr::new(
                    sema.db,
                    power_operator.get_span(),
                    ExprKind::PowerOperator { left, right },
                    sema.current_scope,
                ))
            }
            ast::generated::Expression::UnaryOperator(unary_operator) => {
                let expr = unary_operator.expr.to_expr(sema)?;

                let operator = match unary_operator.operator.deref() {
                    ast::generated::Unary::Token_Plus(_) => UnaryOperatorKind::Plus,
                    ast::generated::Unary::Token_Minus(_) => UnaryOperatorKind::Minus,
                    ast::generated::Unary::Token_NOT(_) => UnaryOperatorKind::Not,
                };

                Ok(Expr::new(
                    sema.db,
                    unary_operator.get_span(),
                    ExprKind::UnaryOperator { expr, operator },
                    sema.current_scope,
                ))
            }
        }
    }
}

impl<'db> ParseExpression<'db> for ast::generated::PrimaryExpression {
    fn to_expr(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        match self {
            ast::generated::PrimaryExpression::ERRInvocationInExprContext(err) => {
                invocation_in_expression(sema.db, err.get_span());
                Err(anyhow::anyhow!(
                    "Invocation in expression context is not allowed"
                ))
            }
            ast::generated::PrimaryExpression::Constant(c) => c.to_expr(sema),
            ast::generated::PrimaryExpression::VariableAccess(v) => {
                let variable = v.variable.to_access(sema)?;
                // todo: add multibits support

                Ok(Expr::new(
                    sema.db,
                    v.get_span(),
                    ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess {
                        variable,
                        multibits: None,
                    }),
                    sema.current_scope,
                ))
            }
            ast::generated::PrimaryExpression::FuncCall(func) => {
                let target = func.function.parse(sema)?;

                let mut parameters = vec![];
                for params in func.params.iter() {
                    match params.deref() {
                        ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                        ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                            match p.children.deref() {
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                                        parameters.push(ParamAssign::ParamAssignInput {
                                            param: p.param.as_ref().map(|p| Ident::from_node(sema.db, sema.file, p.deref())).transpose()?,
                                            value: p.value.to_expr(sema)?,
                                        })
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let variable = p.variable.to_access(sema)?;
                                        parameters.push(ParamAssign::ParamAssignOutput {
                                            not: p.not.is_some(),
                                            param: Ident::from_node(sema.db, sema.file, p.param.deref())?,
                                            variable,
                                        })
                                    }
                                }
                        }
                    }
                }
                Ok(Expr::new(
                    sema.db,
                    func.get_span(),
                    ExprKind::PrimaryExpr(PrimaryExpr::FuncCall {
                        path: PathExpr {
                            span: func.get_span(),
                            expr: target,
                        },
                        params: parameters,
                    }),
                    sema.current_scope,
                ))
            }
            ast::generated::PrimaryExpression::ParenthesizedExpression(p) => Ok(Expr::new(
                sema.db,
                p.get_span(),
                ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr {
                    expr: p.children.to_expr(sema)?,
                }),
                sema.current_scope,
            )),
            ast::generated::PrimaryExpression::RefValue(r) => match r.children.deref() {
                ast::generated::Null_RefAddr::Null(_) => Ok(Expr::new(
                    sema.db,
                    r.get_span(),
                    ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                        value: RefValue::Null,
                    }),
                    sema.current_scope,
                )),
                ast::generated::Null_RefAddr::RefAddr(a) => match a.children.deref() {
                    ast::generated::InstanceName_SymbolicVariable::SymbolicVariable(s) => {
                        Ok(Expr::new(
                            sema.db,
                            s.get_span(),
                            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                                value: RefValue::Address(RefAdress::Symbolic(SymbolicVariable {
                                    this: s.this.is_some(),
                                    kind: s.children.parse(sema)?,
                                })),
                            }),
                            sema.current_scope,
                        ))
                    }
                    ast::generated::InstanceName_SymbolicVariable::InstanceName(i) => {
                        Ok(Expr::new(
                            sema.db,
                            r.get_span(),
                            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                                value: RefValue::Address(RefAdress::Instance(Ident::from_node(
                                    sema.db, sema.file, i,
                                )?)),
                            }),
                            sema.current_scope,
                        ))
                    }
                },
            },
        }
    }
}

pub trait ParseNumeric<'db> {
    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Numeric>;
}

impl<'db> ParseNumeric<'db> for ast::generated::BinaryInt_HexInt_OctalInt_SignedInt {
    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Numeric> {
        Ok(match self {
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::BinaryInt(binary_int) => {
                Numeric::new(
                    sema.db,
                    Ident::from_node(sema.db, sema.file, binary_int)?,
                    NumericKind::Binary,
                )
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::HexInt(hex_int) => Numeric::new(
                sema.db,
                Ident::from_node(sema.db, sema.file, hex_int)?,
                NumericKind::Hex,
            ),
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::OctalInt(octal_int) => {
                Numeric::new(
                    sema.db,
                    Ident::from_node(sema.db, sema.file, octal_int)?,
                    NumericKind::Octal,
                )
            }
            ast::generated::BinaryInt_HexInt_OctalInt_SignedInt::SignedInt(signed_int) => {
                Numeric::new(
                    sema.db,
                    Ident::from_node(sema.db, sema.file, signed_int)?,
                    NumericKind::Signed,
                )
            }
        })
    }
}

impl<'db> ParseExpression<'db> for ast::generated::Constant {
    fn to_expr(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        type Constant = ast::generated::BoolLiteral_CharLiteral_NumericLiteral_TimeLiteral;

        let lit =             match self.children.deref() {
                Constant::BoolLiteral(bool_literal) => {
                    match bool_literal.children.deref() {
                        ast::generated::BoolLiteralWithNumeric_BoolLiteralWithString::BoolLiteralWithNumeric(bool_literal) => {
                            Elementary::Bool(Ident::from_node(sema.db, sema.file, bool_literal.value.deref())?)
                        }
                        ast::generated::BoolLiteralWithNumeric_BoolLiteralWithString::BoolLiteralWithString(bool_literal) => {
                           Elementary::Bool(Ident::from_node(sema.db, sema.file, bool_literal.value.deref())?)
                        }
                    }
                }
                Constant::CharLiteral(char_literal) => {
                    Elementary::AnyString(Ident::from_node(sema.db, sema.file, char_literal.value.deref())?)
                }
                Constant::NumericLiteral(numeric_literal) => {
                    match numeric_literal.children.deref() {
                    ast::generated::IntLiteral_RealLiteral::IntLiteral(int_literal) => {
                        match &int_literal.kind {
                            None => Elementary::InferNumeric(int_literal.int.parse(sema)?),
                            Some(kind) => {
                                match kind.children.deref() {
                                    ast::generated::IntTypeName_MultibitsTypeName::IntTypeName(int_type_name) => {
                                        match int_type_name.children.deref() {
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(sign_int_type_name) => {
                                                match sign_int_type_name.children.deref() {
                                                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Elementary::SInt(int_literal.int.parse(sema)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Elementary::Int(int_literal.int.parse(sema)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Elementary::DInt(int_literal.int.parse(sema)?),
                                                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Elementary::LInt(int_literal.int.parse(sema)?),
                                                }
                                            },
                                            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(unsign_int_type_name) => {
                                                match unsign_int_type_name.children.deref() {
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => Elementary::USInt(int_literal.int.parse(sema)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => Elementary::UInt(int_literal.int.parse(sema)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => Elementary::UDInt(int_literal.int.parse(sema)?),
                                                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => Elementary::ULInt(int_literal.int.parse(sema)?),
                                                }
                                            },
                                        }
                                    },
                                    ast::generated::IntTypeName_MultibitsTypeName::MultibitsTypeName(multibits_type_name) => {
                                        match multibits_type_name.children.deref() {
                                            ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => Elementary::Byte(int_literal.int.parse(sema)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => Elementary::Word(int_literal.int.parse(sema)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => Elementary::DWord(int_literal.int.parse(sema)?),
                                            ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => Elementary::LWord(int_literal.int.parse(sema)?),
                                        }
                                    },
                                }
                            }
                        }
                    },
                    ast::generated::IntLiteral_RealLiteral::RealLiteral(real_literal) => {
                        let j = real_literal.Type.as_deref();

                        match real_literal.Type.as_deref() {
                            Some(kind) => {
                                match kind.children.deref() {
                                    ast::generated::LrealName_RealName::RealName(_) => {
                                        Elementary::Real(Ident::from_node(sema.db, sema.file, real_literal.value.deref())?)
                                    }
                                    ast::generated::LrealName_RealName::LrealName(_) => {
                                        Elementary::LReal(Ident::from_node(sema.db, sema.file, real_literal.value.deref())?)
                                    }
                                }
                            }
                            None => Elementary::InferIdent(Ident::from_node(sema.db, sema.file, real_literal.value.deref())?)
                        }
                    },
                }
                }
                Constant::TimeLiteral(time_literal) => match time_literal.children.deref() {
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::Date(date) => {
                        match date.children.deref() {
                            ast::generated::LongDate_ShortDate::LongDate(date) => {
                                Elementary::LDate(Ident::from_node(sema.db, sema.file, date.value.deref())?)
                            }
                            ast::generated::LongDate_ShortDate::ShortDate(date) => {
                                Elementary::Date(Ident::from_node(sema.db, sema.file, date.value.deref())?)
                            }
                        }
                    }
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::DateAndTime(
                        date_and_time,
                    ) => match date_and_time.children.deref() {
                        ast::generated::LongDateAndTime_ShortDateAndTime::LongDateAndTime(
                            dt,
                        ) => Elementary::LDateTime(Ident::from_node(sema.db, sema.file, dt.value.deref())?),
                        ast::generated::LongDateAndTime_ShortDateAndTime::ShortDateAndTime(
                            dt,
                        ) => Elementary::DateAndTime(Ident::from_node(sema.db, sema.file, dt.value.deref())?),
                    },
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::Duration(duration) => {
                        match duration.children.deref() {
                            ast::generated::Ltime_Time::Ltime(ltime) => {
                                Elementary::LTime(Ident::from_node(sema.db, sema.file, ltime.value.deref())?)
                            }
                            ast::generated::Ltime_Time::Time(time) => {
                                Elementary::Time(Ident::from_node(sema.db, sema.file, time.value.deref())?)
                            }
                        }
                    }
                    ast::generated::Date_DateAndTime_Duration_TimeOfDay::TimeOfDay(time_of_day) => {
                        match time_of_day.children.deref() {
                            ast::generated::Ltod_Tod::Ltod(ltod) => {
                                Elementary::LTod(Ident::from_node(sema.db, sema.file, ltod.value.deref())?)
                            }
                            ast::generated::Ltod_Tod::Tod(tod) => {
                                Elementary::TimeOfDay(Ident::from_node(sema.db, sema.file, tod.value.deref())?)
                            }
                        }
                    }
                },
            };

        Ok(Expr::new(
            sema.db,
            self.get_span(),
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)),
            sema.current_scope,
        ))
    }
}

pub trait ParseVariableAccess<'db> {
    fn to_access(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<VariableAccess<'db>>;
}

impl<'db> ParseVariableAccess<'db> for ast::generated::Variable {
    fn to_access(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<VariableAccess<'db>> {
        match self.children.deref() {
            ast::generated::DirectVariable_SymbolicVariable::DirectVariable(v) => v.to_access(sema),
            ast::generated::DirectVariable_SymbolicVariable::SymbolicVariable(v) => {
                v.to_access(sema)
            }
        }
    }
}

impl<'db> ParseVariableAccess<'db> for ast::generated::DirectVariable {
    fn to_access(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<VariableAccess<'db>> {
        let adress = Ident::from_node(sema.db, sema.file, self.adress.deref())?;

        let (offset, partly) = match self.offset.deref() {
            ast::generated::Offset_Partly::Offset(offset) => {
                (Some(Ident::from_node(sema.db, sema.file, offset)?), false)
            }
            ast::generated::Offset_Partly::Partly(partly) => (None, true),
        };

        Ok(VariableAccess {
            span: self.get_span(),
            kind: VariableAccessKind::Direct {
                adress,
                partly,
                offset,
            },
        })
    }
}

impl<'db> ParseVariableAccess<'db> for ast::generated::SymbolicVariable {
    fn to_access(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<VariableAccess<'db>> {
        Ok(VariableAccess {
            span: self.get_span(),
            kind: VariableAccessKind::Symbolic(SymbolicVariable {
                this: self.this.is_some(),
                kind: self.children.parse(sema)?,
            }),
        })
    }
}

pub trait ParseExpr<'db> {
    type Output;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output>;
}

impl<'db> ParseExpr<'db> for ast::generated::PathExpression {
    type Output = PathExprKind<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        Ok(match self.children.deref() {
            ast::generated::FieldExpression_IndexExpression_VarAccess::FieldExpression(
                field_expr,
            ) => PathExprKind::Field(field_expr.parse(sema)?),
            ast::generated::FieldExpression_IndexExpression_VarAccess::IndexExpression(
                index_expr,
            ) => PathExprKind::Index(index_expr.parse(sema)?),
            ast::generated::FieldExpression_IndexExpression_VarAccess::VarAccess(var_access) => {
                PathExprKind::VarAccess(var_access.parse(sema)?)
            }
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::FieldExpression {
    type Output = FieldExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        Ok(FieldExpr {
            path: Box::new(self.path.parse(sema)?),
            var: self.target.parse(sema)?,
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::IndexExpression {
    type Output = IndexExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        Ok(IndexExpr {
            path: Box::new(self.children.parse(sema)?),
            index: self
                .index
                .children
                .iter()
                .map(|i| i.children.to_expr(sema))
                .collect::<anyhow::Result<Vec<_>>>()?,
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::VarAccess {
    type Output = VarAccess;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        match self.children.deref() {
            ast::generated::ERRUnexpectedThisInPath_Field_RefDeref::ERRUnexpectedThisInPath(
                direct_variable,
            ) => {
                unexpected_this(sema.db, direct_variable.get_span());
                Err(anyhow::anyhow!("Unexpected 'this' in path"))
            }
            ast::generated::ERRUnexpectedThisInPath_Field_RefDeref::Field(field) => Ok(
                VarAccess::Simple(Ident::from_node(sema.db, sema.file, field)?),
            ),
            ast::generated::ERRUnexpectedThisInPath_Field_RefDeref::RefDeref(ref_deref) => Ok(
                VarAccess::Deref(Ident::from_node(sema.db, sema.file, ref_deref.Ref.deref())?),
            ),
        }
    }
}
