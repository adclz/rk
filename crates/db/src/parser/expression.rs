use std::ops::Deref;

use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    default::db::{file::File, BaseDatabase},
};
use salsa::Accumulator;

use crate::hir::expression::{FieldExpr, IndexExpr, PathExpr};
use crate::{
    diagnostics::{diagnostic_builder::diag, DiagnosticAccumulator},
    hir::expression::{
        Expr, ExprKind, Literal, Numeric, Operator, ParamAssign, PathExprKind, PrimaryExpr, RefAdress,
        RefValue, SymbolicVariable, VarAccess, Variable,
    },
    ident::Ident,
};
pub trait ParseExpression<'db> {
    fn to_expr(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>>;
}

impl<'db> ParseExpression<'db> for ast::generated::Expression {
    fn to_expr(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        match self {
            ast::generated::Expression::PrimaryExpression(p) => p.to_expr(db, file),
            ast::generated::Expression::BooleanOperator(boolean_operator) => match boolean_operator
                .children
                .deref()
            {
                ast::generated::AndOperator_OrOperator_XorOperator::OrOperator(or_operator) => {
                    let left = or_operator.left.to_expr(db, file)?;
                    let right = or_operator.right.to_expr(db, file)?;

                    Ok(Expr::new(
                        db,
                        or_operator.get_span(),
                        ExprKind::BooleanOperator {
                            left,
                            operator: Operator::Or,
                            right,
                        }
                    ))
                }
                ast::generated::AndOperator_OrOperator_XorOperator::XorOperator(xor_operator) => {
                    let left = xor_operator.left.to_expr(db, file)?;
                    let right = xor_operator.right.to_expr(db, file)?;

                    Ok(Expr::new(
                        db,
                        xor_operator.get_span(),
                        ExprKind::BooleanOperator {
                            left,
                            operator: Operator::Xor,
                            right,
                        }
                    ))
                }
                ast::generated::AndOperator_OrOperator_XorOperator::AndOperator(and_operator) => {
                    let left = and_operator.left.to_expr(db, file)?;
                    let right = and_operator.right.to_expr(db, file)?;

                    Ok(Expr::new(
                        db,
                        and_operator.get_span(),
                        ExprKind::BooleanOperator {
                            left,
                            operator: Operator::And,
                            right,
                        }
                    ))
                }
            },
            ast::generated::Expression::ComparisonOperator(comparison_operator) => {
                match comparison_operator.children.deref() {
                    ast::generated::EqOperator_OrdOperator::EqOperator(eq_operator) => {
                        let left = eq_operator.left.to_expr(db, file)?;
                        let right = eq_operator.right.to_expr(db, file)?;

                        let operator = match eq_operator.operator.deref() {
                            ast::generated::Eq::Token_Equal(_) => Operator::Eq,
                            ast::generated::Eq::Token_LessGreater(_) => Operator::Ne,
                        };

                        Ok(Expr::new(
                            db,
                            eq_operator.get_span(),
                            ExprKind::ComparisonOperator {
                                left,
                                operator,
                                right,
                            }
                        ))
                    }
                    ast::generated::EqOperator_OrdOperator::OrdOperator(ord_operator) => {
                        let left = ord_operator.left.to_expr(db, file)?;
                        let right = ord_operator.right.to_expr(db, file)?;

                        let operator = match ord_operator.operator.deref() {
                            ast::generated::Ord::Token_Less(_) => Operator::Lt,
                            ast::generated::Ord::Token_Greater(_) => Operator::Gt,
                            ast::generated::Ord::Token_LessEqual(_) => Operator::Le,
                            ast::generated::Ord::Token_GreaterEqual(_) => Operator::Ge,
                        };

                        Ok(Expr::new(
                            db,
                            ord_operator.get_span(),
                            ExprKind::ComparisonOperator {
                                left,
                                operator,
                                right,
                            }
                        ))
                    }
                }
            }
            ast::generated::Expression::AddOperator(add_operator) => {
                let left = add_operator.left.to_expr(db, file)?;
                let right = add_operator.right.to_expr(db, file)?;
                let operator = match add_operator.operator.deref() {
                    ast::generated::Add::Token_Plus(_) => Operator::Plus,
                    ast::generated::Add::Token_Minus(_) => Operator::Minus,
                };

                Ok(Expr::new(
                    db,
                    add_operator.get_span(),
                    ExprKind::AddOperator {
                        left,
                        operator,
                        right,
                    }
                ))
            }
            ast::generated::Expression::MultOperator(mult_operator) => {
                let left = mult_operator.left.to_expr(db, file)?;
                let right = mult_operator.right.to_expr(db, file)?;
                let operator = match mult_operator.operator.deref() {
                    ast::generated::Mult::Token_Star(_) => Operator::Mul,
                    ast::generated::Mult::Token_Slash(_) => Operator::Div,
                    ast::generated::Mult::Token_MOD(_) => Operator::Mod,
                };

                Ok(Expr::new(
                    db,
                    mult_operator.get_span(),
                    ExprKind::MultOperator {
                        left,
                        operator,
                        right,
                    }
                ))
            }
            ast::generated::Expression::PowerOperator(power_operator) => {
                let left = power_operator.left.to_expr(db, file)?;
                let right = power_operator.right.to_expr(db, file)?;

                Ok(Expr::new(
                    db,
                    power_operator.get_span(),
                    ExprKind::PowerOperator { left, right },
                ))
            }
            ast::generated::Expression::UnaryOperator(unary_operator) => {
                let expr = unary_operator.expr.to_expr(db, file)?;

                let operator = match unary_operator.operator.deref() {
                    ast::generated::Unary::Token_Plus(_) => Operator::Plus,
                    ast::generated::Unary::Token_Minus(_) => Operator::Minus,
                    ast::generated::Unary::Token_NOT(_) => Operator::Not,
                };

                Ok(Expr::new(
                    db,
                    unary_operator.get_span(),
                    ExprKind::UnaryOperator { expr, operator },
                ))
            }
        }
    }
}

impl<'db> ParseExpression<'db> for ast::generated::PrimaryExpression {
    fn to_expr(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        match self {
            ast::generated::PrimaryExpression::ERRInvocationInExprContext(err) => {
                let diag = diag()
                    .file(file)
                    .message("Invocation in expression context is not allowed".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .range(err.get_span())
                    .call();
                DiagnosticAccumulator::accumulate(diag.into(), db);
                Err(anyhow::anyhow!(
                    "Invocation in expression context is not allowed"
                ))
            }
            ast::generated::PrimaryExpression::Constant(c) => c.to_expr(db, file),
            ast::generated::PrimaryExpression::VariableAccess(v) => {
                todo!()
            }
            ast::generated::PrimaryExpression::FuncCall(func) => {
                let target = func.function.parse(db, file)?;

                let mut parameters = vec![];
                for params in func.params.iter() {
                    match params.deref() {
                        ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                        ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                            match p.children.deref() {
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                                        parameters.push(ParamAssign::ParamAssignInput {
                                            param: p.param.as_ref().map(|p| Ident::from_node(db, file, p.deref())).transpose()?,
                                            value: p.value.to_expr(db, file)?,
                                        })
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let variable = p.variable.to_access(db, file)?;
                                        
                                        parameters.push(ParamAssign::ParamAssignOutput {
                                            not: p.not.is_some(),
                                            param: Ident::from_node(db, file, p.param.deref())?,
                                            variable,
                                        })
                                    }
                                }
                        }
                    }
                }
                Ok(Expr::new(
                    db,
                    func.get_span(),
                    ExprKind::PrimaryExpr(PrimaryExpr::FuncCall {
                        path: PathExpr::new(db, func.get_span(), target),
                        params: parameters,
                    }),
                ))
            }
            ast::generated::PrimaryExpression::ParenthesizedExpression(p) => Ok(Expr::new(
                db,
                p.get_span(),
                ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr {
                    expr: p.children.to_expr(db, file)?,
                }),
            )),
            ast::generated::PrimaryExpression::RefValue(r) => match r.children.deref() {
                ast::generated::Null_RefAddr::Null(_) => Ok(Expr::new(
                    db,
                    r.get_span(),
                    ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                        value: RefValue::Null,
                    }),
                )),
                ast::generated::Null_RefAddr::RefAddr(a) => match a.children.deref() {
                    ast::generated::InstanceName_SymbolicVariable::SymbolicVariable(s) => {
                        todo!()
                    }
                    ast::generated::InstanceName_SymbolicVariable::InstanceName(i) => {
                        Ok(Expr::new(
                            db,
                            r.get_span(),
                            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                                value: RefValue::Address(RefAdress::Instance(Ident::from_node(
                                    db, file, i,
                                )?)),
                            }),
                        ))
                    }
                },
            },
        }
    }
}

pub trait ParseNumeric<'db> {
    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Numeric>;
}

impl<'db> ParseNumeric<'db> for ast::generated::BinaryInt_HexInt_OctalInt_SignedInt {
    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Numeric> {
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

impl<'db> ParseExpression<'db> for ast::generated::Constant {
    fn to_expr(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        type Constant = ast::generated::BoolLiteral_CharLiteral_NumericLiteral_TimeLiteral;

        let lit =             match self.children.deref() {
                Constant::BoolLiteral(bool_literal) => {
                    match bool_literal.children.deref() {
                        ast::generated::BoolLiteralWithNumeric_BoolLiteralWithString::BoolLiteralWithNumeric(bool_literal) => {
                            Literal::Bool(Ident::from_node(db, file, bool_literal.value.deref())?)
                        }
                        ast::generated::BoolLiteralWithNumeric_BoolLiteralWithString::BoolLiteralWithString(bool_literal) => {
                            Literal::Bool(Ident::from_node(db, file, bool_literal.value.deref())?)
                        }
                    }
                } 
                Constant::CharLiteral(char_literal) => { 
                    Literal::Char(Ident::from_node(db, file, char_literal.value.deref())?)
                }
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
                        let j = real_literal.Type.as_deref();

                        match real_literal.Type.as_deref() {
                            Some(kind) => {
                                match kind.children.deref() {
                                    ast::generated::LrealName_RealName::RealName(_) => {
                                        Literal::Real(Ident::from_node(db, file, real_literal.value.deref())?)
                                    }
                                    ast::generated::LrealName_RealName::LrealName(_) => {
                                        Literal::LReal(Ident::from_node(db, file, real_literal.value.deref())?)
                                    }
                                }
                            }
                            None => Literal::LReal(Ident::from_node(db, file, real_literal.value.deref())?),
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
            };

        Ok(Expr::new(db, self.get_span(),
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)),
        ))
    }
}

pub trait ParseVariableAccess<'db> {
    fn to_access(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Variable<'db>>;
}

impl<'db> ParseVariableAccess<'db> for ast::generated::Variable {
    fn to_access(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Variable<'db>> {
        match self.children.deref() {
            ast::generated::DirectVariable_SymbolicVariable::DirectVariable(v) => {
                v.to_access(db, file)
            }
            ast::generated::DirectVariable_SymbolicVariable::SymbolicVariable(v) => {
                v.to_access(db, file)
            }
        }
    }
}

impl<'db> ParseVariableAccess<'db> for ast::generated::DirectVariable {
    fn to_access(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Variable<'db>> {
        let adress = Ident::from_node(db, file, self.adress.deref())?;

        let (offset, partly) = match self.offset.deref() {
            ast::generated::Offset_Partly::Offset(offset) => {
                (Some(Ident::from_node(db, file, offset)?), false)
            }
            ast::generated::Offset_Partly::Partly(partly) => (None, true),
        };

        Ok(Variable::Direct {
            adress,
            partly,
            offset,
        })
    }
}

impl<'db> ParseVariableAccess<'db> for ast::generated::SymbolicVariable {
    fn to_access(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Variable<'db>> {
        Ok(Variable::Symbolic(SymbolicVariable {
            this: self.this.is_some(),
            kind: self.children.parse(db, file)?,
        }))
    }
}

trait ParseExpr<'db> {
    type Output;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output>;
}

impl<'db> ParseExpr<'db> for ast::generated::PathExpression {
    type Output = PathExprKind<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        Ok(match self.children.deref() {
            ast::generated::FieldExpression_IndexExpression_VarAccess::FieldExpression(field_expr) => {
                PathExprKind::Field(field_expr.parse(db, file)?) 
            }
            ast::generated::FieldExpression_IndexExpression_VarAccess::IndexExpression(index_expr) => {
                PathExprKind::Index(index_expr.parse(db, file)?) 
            }
            ast::generated::FieldExpression_IndexExpression_VarAccess::VarAccess(var_access) => {
                PathExprKind::VarAccess(var_access.parse(db, file)?)
            }
        })
    }
} 

impl<'db> ParseExpr<'db> for ast::generated::FieldExpression {
    type Output = FieldExpr<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        Ok(FieldExpr::new(db, self.path.parse(db, file)?, self.target.parse(db, file)?))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::IndexExpression {
    type Output = IndexExpr<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        Ok(IndexExpr::new(
            db,
            self.children.parse(db, file)?,
            self.index.children
                .iter()
                .map(|i| i.children.to_expr(db, file))
                .collect::<anyhow::Result<Vec<_>>>()?,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::VarAccess {
    type Output = VarAccess;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        match self.children.deref() {
            ast::generated::ERRUnexpectedThisInPath_Field_RefDeref::ERRUnexpectedThisInPath(direct_variable) => {
                    let diag = diag()
                        .file(file)
                        .message("Unexpected 'this' in path".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(direct_variable.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                
                Err(anyhow::anyhow!("Unexpected 'this' in path"))
            }
            ast::generated::ERRUnexpectedThisInPath_Field_RefDeref::Field(field) => {
                Ok(VarAccess::Simple(Ident::from_node(db, file, field)?))
            }
            ast::generated::ERRUnexpectedThisInPath_Field_RefDeref::RefDeref(ref_deref) => {
                Ok(VarAccess::Deref(Ident::from_node(db, file, ref_deref.Ref.deref())?))
            }
        }
    }
}
