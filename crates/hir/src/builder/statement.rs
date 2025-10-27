use crate::builder::expression::{ParseExpr, ParseExpression, ParseVariableAccess};
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::analysis_error::AnalysisError;
use crate::check::errors::syntax::SyntaxError;
use crate::hir_def::expressions::expression::{FuncCall, ParamAssign, ParamAssignKind};
use crate::hir_def::expressions::statement::{CaseKind, Stmt, StmtKind};
use crate::hir_def::interned::identifier::SpanIdent;
use auto_lsp::anyhow::{self};
use auto_lsp::core::ast::AstNode;

pub trait ParseStatement<'db> {
    fn to_statement(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Stmt<'db>, AnalysisError<'db>>;
} 

impl<'db> ParseStatement<'db> for ast::generated::Stmt {
    fn to_statement(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Stmt<'db>, AnalysisError<'db>> {
        type StmtType = ast::generated::Stmt;
        match self {
            StmtType::BeginPathExpression(p) => Ok(Stmt::new(
                sema.db,
                StmtKind::EmptyPathExpression(p.parse(sema)?),
                p.into(),
                sema.current_scope,
            )),
            StmtType::Assign(assign) => assign.to_statement(sema),
            StmtType::FuncCall(call) => {
                let target = call.function.cast(sema.ast).parse(sema)?;
                let mut parameters = vec![];
                for params in call.params.iter() {
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
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::FuncCall(FuncCall::new(sema.db, target, parameters)),
                    call.into(),
                    sema.current_scope,
                ))
            }
            StmtType::IfStmt(if_stmt) => {
                let condition = if_stmt.if_cond.cast(sema.ast).to_expr(sema)?;
                let then = if_stmt
                    .if_body
                    .as_ref()
                    .map(|b| {
                        b.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
                    })
                    .transpose()?;

                let else_if = if_stmt
                    .else_if
                    .iter()
                    .map(|else_if_stmt| {
                        let condition = else_if_stmt
                            .cast(sema.ast)
                            .else_if_cond
                            .cast(sema.ast)
                            .to_expr(sema)?;
                        let then = else_if_stmt
                            .cast(sema.ast)
                            .else_if_body
                            .cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()?;
                        Ok((condition, then))
                    })
                    .collect::<Result<Vec<_>, AnalysisError<'db>>>()?;

                let else_ = if_stmt
                    .else_body
                    .as_ref()
                    .map(|b| {
                        b.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
                    })
                    .transpose()?;

                Ok(Stmt::new(
                    sema.db,
                    StmtKind::If {
                        condition,
                        then,
                        else_,
                        else_if,
                    },
                    if_stmt.into(),
                    sema.current_scope,
                ))
            }
            StmtType::ForStmt(for_stmt) => {
                let control_variable = for_stmt.control_variable.cast(sema.ast).to_access(sema)?;

                let for_list = for_stmt.control_list.cast(sema.ast);

                for_list.children.as_ref().map(|err| {
                    match err.cast(sema.ast) {
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl::ERRMissingDotInForControl(err) => {
                            sema.errors.push(AnalysisError::SyntaxError(SyntaxError::MissingDotInForList {
                                file: sema.file,
                                span: err.get_span(),
                            }))
                        }
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl::ERRMissingEqualInForControl(err) => {
                            sema.errors.push(AnalysisError::SyntaxError(SyntaxError::MissingEqualInForList {
                                file: sema.file,
                                span: err.get_span(),
                            }))
                        }
                    }
                });

                let start = for_list.initial_value.cast(sema.ast).to_expr(sema)?;

                let end = for_stmt
                    .control_list
                    .cast(sema.ast)
                    .end_value
                    .cast(sema.ast)
                    .to_expr(sema)?;
                let step = for_stmt
                    .control_list
                    .cast(sema.ast)
                    .step
                    .as_ref()
                    .map(|s| s.cast(sema.ast).to_expr(sema))
                    .transpose()?;

                let body = for_stmt
                    .body
                    .as_ref()
                    .map(|body| {
                        body.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
                    })
                    .transpose()?
                    .unwrap_or_else(std::vec::Vec::new);

                Ok(Stmt::new(
                    sema.db,
                    StmtKind::For {
                        control_variable,
                        start,
                        end,
                        step,
                        body,
                    },
                    for_stmt.into(),
                    sema.current_scope,
                ))
            }
            StmtType::CaseStmt(case_stmt) => {
                let condition = case_stmt.case_cond.cast(sema.ast).to_expr(sema)?;

                let mut cases = vec![];
                for case in case_stmt.case_selection.iter() {
                    let mut case_of = vec![];

                    for case in &case.cast(sema.ast).case_of.cast(sema.ast).children {
                        match case.cast(sema.ast).children.cast(sema.ast) {
                            ast::generated::ConstantExpr_Subrange::ConstantExpr(constant) => {
                                case_of.push(CaseKind::Expression(
                                    constant.children.cast(sema.ast).to_expr(sema)?,
                                ));
                            }
                            ast::generated::ConstantExpr_Subrange::Subrange(subrange) => {
                                let lower = subrange
                                    .lower
                                    .cast(sema.ast)
                                    .children
                                    .cast(sema.ast)
                                    .to_expr(sema)?;
                                let upper = subrange
                                    .upper
                                    .cast(sema.ast)
                                    .children
                                    .cast(sema.ast)
                                    .to_expr(sema)?;
                                case_of.push(CaseKind::Subrange { lower, upper });
                            }
                        }
                    }

                    let mut body = vec![];
                    if let Some(case_body) = &case.cast(sema.ast).case_do {
                        body = case_body
                            .cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()?;
                    }
                    cases.push((case_of, body));
                }

                let else_ = case_stmt
                    .default
                    .as_ref()
                    .map(|b| {
                        b.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
                    })
                    .transpose()?;

                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Case {
                        condition,
                        cases,
                        else_,
                    },
                    case_stmt.into(),
                    sema.current_scope,
                ))
            }
            StmtType::RepeatStmt(repeat) => {
                let body = repeat
                    .repeat_body
                    .as_ref()
                    .map(|body| {
                        body.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
                    })
                    .transpose()?
                    .unwrap_or_else(std::vec::Vec::new);

                let condition = repeat.repeat_cond.cast(sema.ast).to_expr(sema)?;
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Repeat { body, condition },
                    repeat.into(),
                    sema.current_scope,
                ))
            }
            StmtType::WhileStmt(while_stmt) => {
                let condition = while_stmt.while_cond.cast(sema.ast).to_expr(sema)?;
                let body = while_stmt
                    .while_body
                    .as_ref()
                    .map(|body| {
                        body.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).to_statement(sema))
                            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
                    })
                    .transpose()?
                    .unwrap_or_else(std::vec::Vec::new);

                Ok(Stmt::new(
                    sema.db,
                    StmtKind::While { condition, body },
                    while_stmt.into(),
                    sema.current_scope,
                ))
            }
            StmtType::Token_RETURN(return_stmt) => Ok(Stmt::new(
                sema.db,
                StmtKind::Return,
                return_stmt.into(),
                sema.current_scope,
            )),
            StmtType::Token_CONTINUE(stmt) => Ok(Stmt::new(
                sema.db,
                StmtKind::Continue,
                stmt.into(),
                sema.current_scope,
            )),
            StmtType::Token_EXIT(stmt) => Ok(Stmt::new(
                sema.db,
                StmtKind::Exit,
                stmt.into(),
                sema.current_scope,
            )),
        }
    }
}

impl<'db> ParseStatement<'db> for ast::generated::Assign {
    fn to_statement(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Stmt<'db>, AnalysisError<'db>> {
        let var = match self.variable.cast(sema.ast) {
            ast::generated::ERRAssignFuncCall_Variable::ERRAssignFuncCall(err) => Err(
                AnalysisError::SyntaxError(SyntaxError::AssignToFunctionCall(err.get_span())),
            ),
            ast::generated::ERRAssignFuncCall_Variable::Variable(var) => var.to_access(sema),
        }?;

        type TargetType = ast::generated::ERREmptyRightHandAssignment_ERRMissingDotInAssignment_ERRMissingEqualInAssignment_Assignment_AssignmentAttempt;
        match self.target.cast(sema.ast) {
            TargetType::ERREmptyRightHandAssignment(err) => Err(AnalysisError::SyntaxError(
                SyntaxError::EmptyRightHandSide(err.get_span()),
            )),
            TargetType::ERRMissingDotInAssignment(err) => Err(AnalysisError::SyntaxError(
                SyntaxError::MissingDotInAssignment {
                    file: sema.file,
                    span: err.get_span(),
                },
            )),
            TargetType::ERRMissingEqualInAssignment(err) => Err(AnalysisError::SyntaxError(
                SyntaxError::MissingEqualInAssignment {
                    file: sema.file,
                    span: err.get_span(),
                },
            )),
            TargetType::Assignment(assign) => Ok(Stmt::new(
                sema.db,
                StmtKind::Assignment {
                    var,
                    target: assign.children.cast(sema.ast).to_expr(sema)?,
                },
                self.into(),
                sema.current_scope,
            )),
            TargetType::AssignmentAttempt(attempt) => Ok(Stmt::new(
                sema.db,
                StmtKind::AssignmentAttempt {
                    var,
                    target: attempt.children.cast(sema.ast).to_expr(sema)?,
                },
                self.into(),
                sema.current_scope,
            )),
        }
    }
}
