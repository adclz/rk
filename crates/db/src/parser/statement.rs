use crate::check::errors::semantic_errors::{assign_to_function_call, empty_right_hand_assignment};
use crate::hir::expressions::expression::{ParamAssign, SymbolicVariable};
use crate::hir::expressions::statement::{CaseKind, Stmt, StmtKind};
use crate::hir::interned::identifier::Ident;
use crate::parser::expression::{ParseExpr, ParseExpression, ParseVariableAccess};
use crate::parser::semantic_index::SemanticIndexBuilder;
use auto_lsp::anyhow::{self, Ok};
use auto_lsp::core::ast::AstNode;

pub trait ParseStatement<'db> {
    fn to_statement(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Stmt<'db>>;
}

impl<'db> ParseStatement<'db> for ast::generated::Stmt {
    fn to_statement(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Stmt<'db>> {
        type StmtType = ast::generated::Stmt;
        match self {
            StmtType::Assign(assign) => assign.to_statement(sema),
            StmtType::FuncCall(call) => {
                let target = call.function.cast(&sema.ast).parse(sema)?;
                let mut parameters = vec![];
                for params in call.params.iter() {
                    match params.cast(&sema.ast) {
                        ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                        ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                            match p.children.cast(&sema.ast) {
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                                        parameters.push(ParamAssign::ParamAssignInput {
                                            param: p.param.as_ref().map(|p| Ident::from_node(sema.db, sema.file, p.cast(&sema.ast))).transpose()?,
                                            value: p.value.cast(&sema.ast).to_expr(sema)?,
                                        })
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let variable = p.variable.cast(&sema.ast).to_access(sema)?;
                                        parameters.push(ParamAssign::ParamAssignOutput {
                                            not: p.not.is_some(),
                                            param: Ident::from_node(sema.db, sema.file, p.param.cast(&sema.ast))?,
                                            variable,
                                        })
                                    }
                                }
                        }
                    }
                }
                Ok(Stmt::new(
                    sema.db,
                    call.get_span(),
                    StmtKind::FuncCall { target, params: parameters },
                ))
            }
            StmtType::Invocation(invocation) => {
                let mut parameters = vec![];
                for params in invocation.params.iter() {
                    match params.cast(&sema.ast) {
                        ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                        ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                            match p.children.cast(&sema.ast) {
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                                        parameters.push(ParamAssign::ParamAssignInput {
                                            param: p.param.as_ref().map(|p| Ident::from_node(sema.db, sema.file, p.cast(&sema.ast))).transpose()?,
                                            value: p.value.cast(&sema.ast).to_expr(sema)?,
                                        })
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let variable = p.variable.cast(&sema.ast).to_access(sema)?;
                                        parameters.push(ParamAssign::ParamAssignOutput {
                                            not: p.not.is_some(),
                                            param: Ident::from_node(sema.db, sema.file, p.param.cast(&sema.ast))?,
                                            variable,
                                        })
                                    }
                                }
                        }
                    }
                };
                Ok(Stmt::new(sema.db, invocation.get_span(), StmtKind::Invocation { 
                    target: SymbolicVariable {
                        this: invocation.invocation.cast(&sema.ast).this.is_some(),
                        kind: invocation.invocation.cast(&sema.ast).children.cast(&sema.ast).parse(sema)?,
                    }, 
                    params: parameters
                }))
            }
            StmtType::IfStmt(if_stmt) => {
                let condition = if_stmt.if_cond.cast(&sema.ast).to_expr(sema)?;
                let then = if_stmt.if_body
                    .as_ref()
                    .map(|b| b.cast(&sema.ast).children.iter()
                        .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                        .collect::<anyhow::Result<Vec<_>>>())
                    .transpose()?;  

                let else_if = if_stmt
                    .else_if
                    .iter()
                    .map(|else_if_stmt| {
                        let condition = else_if_stmt.cast(&sema.ast).else_if_cond.cast(&sema.ast).to_expr(sema)?;
                        let then = else_if_stmt.cast(&sema.ast).else_if_body.cast(&sema.ast).children
                            .iter()
                            .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                            .collect::<anyhow::Result<Vec<_>>>()?;
                    Ok((condition, then)) 
                }).collect::<anyhow::Result<Vec<_>>>()?; 
 
                let else_ = if_stmt
                    .else_body
                    .as_ref().map(|b| b
                            .cast(&sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                            .collect::<anyhow::Result<Vec<_>>>()
                    ).transpose()?;

                Ok(Stmt::new( 
                    sema.db,
                    if_stmt.get_span(),
                    StmtKind::If {
                        condition,
                        then,
                        else_,
                        else_if
                    },
                ))
            }
            StmtType::ForStmt(for_stmt) => {
                let control_variable = for_stmt.control_variable.cast(&sema.ast).to_access(sema)?;
                
                let start = for_stmt.control_list.cast(&sema.ast).initial_value.cast(&sema.ast).to_expr(sema)?;
                let end = for_stmt.control_list.cast(&sema.ast).end_value.cast(&sema.ast).to_expr(sema)?;
                let step = for_stmt.control_list.cast(&sema.ast).step.as_ref().map(|s| s.cast(&sema.ast).to_expr(sema)).transpose()?;

                let body = for_stmt.body
                    .as_ref()
                    .map(|body| {
                        body.cast(&sema.ast).children
                            .iter()
                            .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                            .collect::<anyhow::Result<Vec<_>>>()
                    }).transpose()?
                    .unwrap_or_else(|| vec![]);

                Ok(Stmt::new(
                    sema.db,
                    for_stmt.get_span(),
                    StmtKind::For {
                        control_variable,
                        start,
                        end,
                        step,
                        body,
                    }, 
                )) 
            }
            StmtType::CaseStmt(case_stmt) => {
                let condition = case_stmt.case_cond.cast(&sema.ast).to_expr(sema)?;

                let mut cases = vec![];
                for case in case_stmt.case_selection.iter() {
                    let mut case_of = vec![];

                    for case in &case.cast(&sema.ast).case_of.cast(&sema.ast).children {
                        match case.cast(&sema.ast).children.cast(&sema.ast) {
                            ast::generated::ConstantExpr_Subrange::ConstantExpr(constant) => {
                                case_of.push(CaseKind::Expression(constant.children.cast(&sema.ast).to_expr(sema)? 
                            ));
                                
                            }
                            ast::generated::ConstantExpr_Subrange::Subrange(subrange) => {
                                let lower = subrange.lower.cast(&sema.ast).children.cast(&sema.ast).to_expr(sema)?;
                                let upper = subrange.upper.cast(&sema.ast).children.cast(&sema.ast).to_expr(sema)?;
                                case_of.push(CaseKind::Subrange { lower, upper });
                            }
                        }
                    }

                    let mut body = vec![];
                    if let Some(case_body) = &case.cast(&sema.ast).case_do {
                        body = case_body
                            .cast(&sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                            .collect::<anyhow::Result<Vec<_>>>()?;
                    }
                    cases.push((case_of, body));
                }

                let else_ = case_stmt
                    .default
                    .as_ref()
                    .map(|b| b
                        .cast(&sema.ast)
                        .children
                        .iter()
                        .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                        .collect::<anyhow::Result<Vec<_>>>()
                    ).transpose()?;

                Ok(Stmt::new(
                    sema.db,
                    case_stmt.get_span(),
                    StmtKind::Case {
                        condition,
                        cases,
                        else_,
                    },
                ))
            }
            StmtType::RepeatStmt(repeat) => {
                let body = repeat
                    .repeat_body
                    .cast(&sema.ast)
                    .children
                    .iter()
                    .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                    .collect::<anyhow::Result<Vec<_>>>()?;

                let condition = repeat.repeat_cond.cast(&sema.ast).to_expr(sema)?;
                Ok(Stmt::new(
                    sema.db,
                    repeat.get_span(),
                    StmtKind::Repeat { body, condition },
                ))
            }
            StmtType::WhileStmt(while_stmt) => {
                let condition = while_stmt.while_cond.cast(&sema.ast).to_expr(sema)?;
                let body = while_stmt
                    .while_body
                    .cast(&sema.ast)
                    .children
                    .iter()
                    .map(|stmt| stmt.cast(&sema.ast).to_statement(sema))
                    .collect::<anyhow::Result<Vec<_>>>()?;

                Ok(Stmt::new(
                    sema.db,
                    while_stmt.get_span(),
                    StmtKind::While { condition, body },
                ))
            }
            StmtType::SuperStmt(super_stmt) => {
                Ok(Stmt::new(sema.db, super_stmt.get_span(), StmtKind::Super))
            }
            StmtType::Token_RETURN(return_stmt) => {
                Ok(Stmt::new(sema.db, return_stmt.get_span(), StmtKind::Return))
            }
            StmtType::Token_CONTINUE(stmt) => {
                Ok(Stmt::new(sema.db, stmt.get_span(), StmtKind::Continue))
            }
            StmtType::Token_EXIT(stmt) => Ok(Stmt::new(sema.db, stmt.get_span(), StmtKind::Exit)),
        }
    }
}

impl<'db> ParseStatement<'db> for ast::generated::Assign {
    fn to_statement(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Stmt<'db>> {
        let var = match self.variable.cast(&sema.ast) {
            ast::generated::ERRAssignFuncCall_Variable::ERRAssignFuncCall(err) => {
                assign_to_function_call(sema.db, err.get_span());
                Err(anyhow::anyhow!("Cannot assign to a function call"))
            }
            ast::generated::ERRAssignFuncCall_Variable::Variable(var) => var.to_access(sema),
        }?;

        match self.target.cast(&sema.ast) {
            ast::generated::ERREmptyRightHandAssignment_Assignment_AssignmentAttempt::ERREmptyRightHandAssignment(err) => {
                empty_right_hand_assignment(sema.db, err.get_span());
                Err(anyhow::anyhow!("Empty right-hand side in assignment"))
            },
            ast::generated::ERREmptyRightHandAssignment_Assignment_AssignmentAttempt::Assignment(assign) => Ok(Stmt::new(
                sema.db,
                assign.get_span(),
                StmtKind::Assignment {
                    var,
                    target: assign.children.cast(&sema.ast).to_expr(sema)?,
                },
            )), 
            ast::generated::ERREmptyRightHandAssignment_Assignment_AssignmentAttempt::AssignmentAttempt(attempt) => {    
                Ok(Stmt::new(
                    sema.db,
                    attempt.get_span(),
                    StmtKind::AssignmentAttempt {
                        var,
                        target: attempt.children.cast(&sema.ast).to_expr(sema)?,
                    },
                ))
            }
        }
    }
}
