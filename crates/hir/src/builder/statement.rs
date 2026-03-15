use crate::builder::Parse;
use crate::builder::ParseSpec;
use crate::builder::expression::ParseVariableAccess;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::expressions::expression::{FuncCall, ParamAssignKind};
use crate::hir_def::expressions::statement::{CaseKind, Stmt, StmtKind};
use crate::hir_def::interned::identifier::SpanIdent;
use auto_lsp::anyhow::{self};
use auto_lsp::core::ast::AstNode;
use ide_diagnostic::IdeDiagnostic;

impl<'db> Parse<'db> for ast::generated::Stmt {
    type Output = Stmt<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Stmt<'db>, IdeDiagnostic> {
        type StmtType = ast::generated::Stmt;
        match self {
            StmtType::BeginPathExpression(p) => Ok(Stmt::new(
                sema.db,
                StmtKind::EmptyPathExpression(p.parse(sema)?),
                p.into(),
                sema.current_scope,
            )),
            StmtType::Assign(assign) => assign.parse(sema),
            StmtType::FuncCall(call) => {
                let target = call.function.cast(sema.ast).parse(sema)?;

                // Parse type arguments if present
                let mut type_args = vec![];
                if let Some(generic_args) = &call.type_args {
                    let generic_args_node = generic_args.cast(sema.ast);
                    for type_arg_id in &generic_args_node.type_arg {
                        type_args.push(type_arg_id.cast(sema.ast).to_spec(sema)?);
                    }
                }

                let mut parameters = vec![];
                for params in call.params.iter() {
                    match params.cast(sema.ast) {
                        ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                        ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                            match p.children.cast(sema.ast) {
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                                        let kind = match p.param.as_ref() {
                                            Some(param) => {
                                                ParamAssignKind::FormalInput { param: SpanIdent::from_node(sema.db, sema, param.cast(sema.ast))?, value: p.value.cast(sema.ast).parse(sema)? }
                                            },
                                            None => {
                                                ParamAssignKind::NonFormal { value: p.value.cast(sema.ast).parse(sema)? }
                                            }
                                        };
                                        parameters.push(sema.new_param(p.into(), sema.current_scope, kind))
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let variable = p.variable.cast(sema.ast).to_access(sema)?;
                                        let kind = ParamAssignKind::FormalOutput { not: p.not.is_some() , param: SpanIdent::from_node(sema.db, sema, p.param.cast(sema.ast))?, variable };
                                        parameters.push(sema.new_param(p.into(), sema.current_scope, kind))
                                    }
                                }
                        }
                    }
                }
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::FuncCall(FuncCall::new(sema.db, target, type_args, parameters)),
                    call.into(),
                    sema.current_scope,
                ))
            }
            StmtType::IfStmt(if_stmt) => {
                let condition = if_stmt.if_cond.cast(sema.ast).parse(sema)?;
                let then = if_stmt
                    .if_body
                    .as_ref()
                    .map(|b| {
                        b.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()
                    })
                    .transpose()?;

                let else_if = if_stmt
                    .else_if
                    .iter()
                    .map(|else_if_stmt| {
                        let else_if_node = else_if_stmt.cast(sema.ast);
                        let condition = else_if_node.else_if_cond.cast(sema.ast).parse(sema)?;
                        let then = else_if_node
                            .else_if_body
                            .as_ref()
                            .map(|b| {
                                b.cast(sema.ast)
                                    .children
                                    .iter()
                                    .map(|stmt| stmt.cast(sema.ast).parse(sema))
                                    .collect::<Result<Vec<_>, IdeDiagnostic>>()
                            })
                            .transpose()?
                            .unwrap_or_default();
                        Ok((condition, then))
                    })
                    .collect::<Result<Vec<_>, IdeDiagnostic>>()?;

                let else_ = if_stmt
                    .else_body
                    .as_ref()
                    .map(|b| {
                        b.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()
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
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl_ERROutputAssignInForControl::ERRMissingDotInForControl(err) => {
                            sema.errors.push(SyntaxError::MissingDotInForList {
                                file: sema.file,
                                span: err.get_span(),
                            }.to_diagnostic(sema.db))
                        }
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl_ERROutputAssignInForControl::ERRMissingEqualInForControl(err) => {
                            sema.errors.push(SyntaxError::MissingEqualInForList {
                                file: sema.file,
                                span: err.get_span(),
                            }.to_diagnostic(sema.db))
                        }
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl_ERROutputAssignInForControl::ERROutputAssignInForControl(err) => {
                            sema.errors.push(SyntaxError::OutputAssignInForList {
                                file: sema.file,
                                span: err.get_span(),
                            }.to_diagnostic(sema.db))
                        }
                    }
                });

                let start = for_list.initial_value.cast(sema.ast).parse(sema)?;

                let end = for_stmt
                    .control_list
                    .cast(sema.ast)
                    .end_value
                    .cast(sema.ast)
                    .parse(sema)?;
                let step = for_stmt
                    .control_list
                    .cast(sema.ast)
                    .step
                    .as_ref()
                    .map(|s| s.cast(sema.ast).parse(sema))
                    .transpose()?;

                let body = for_stmt
                    .body
                    .as_ref()
                    .map(|body| {
                        body.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()
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
                let condition = case_stmt.case_cond.cast(sema.ast).parse(sema)?;

                let mut cases = vec![];
                for case in case_stmt.case_selection.iter() {
                    let mut case_of = vec![];

                    for case in &case.cast(sema.ast).case_of.cast(sema.ast).children {
                        match case.cast(sema.ast).children.cast(sema.ast) {
                            ast::generated::ConstantExpr_Subrange::ConstantExpr(constant) => {
                                case_of.push(CaseKind::Expression(
                                    constant.children.cast(sema.ast).parse(sema)?,
                                ));
                            }
                            ast::generated::ConstantExpr_Subrange::Subrange(subrange) => {
                                let lower = subrange
                                    .lower
                                    .cast(sema.ast)
                                    .children
                                    .cast(sema.ast)
                                    .parse(sema)?;
                                let upper = subrange
                                    .upper
                                    .cast(sema.ast)
                                    .children
                                    .cast(sema.ast)
                                    .parse(sema)?;
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
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()?;
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
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()
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
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()
                    })
                    .transpose()?
                    .unwrap_or_else(std::vec::Vec::new);

                let condition = repeat.repeat_cond.cast(sema.ast).parse(sema)?;
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Repeat { body, condition },
                    repeat.into(),
                    sema.current_scope,
                ))
            }
            StmtType::WhileStmt(while_stmt) => {
                let condition = while_stmt.while_cond.cast(sema.ast).parse(sema)?;
                let body = while_stmt
                    .while_body
                    .as_ref()
                    .map(|body| {
                        body.cast(sema.ast)
                            .children
                            .iter()
                            .map(|stmt| stmt.cast(sema.ast).parse(sema))
                            .collect::<Result<Vec<_>, IdeDiagnostic>>()
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

impl<'db> Parse<'db> for ast::generated::Assign {
    type Output = Stmt<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Stmt<'db>, IdeDiagnostic> {
        let var = match self.variable.cast(sema.ast) {
            ast::generated::ERRAssignFuncCall_VariableAccess::ERRAssignFuncCall(err) => {
                Err(SyntaxError::AssignToFunctionCall(err.get_span()).to_diagnostic(sema.db))
            }
            ast::generated::ERRAssignFuncCall_VariableAccess::VariableAccess(var) => {
                var.to_access(sema)
            }
        }?;

        type TargetType = ast::generated::ERREmptyRightHandAssignment_ERRMissingDotInAssignment_ERRMissingEqualInAssignment_ERROutputAssignInAssignment_Assignment_AssignmentAttempt;
        match self.target.cast(sema.ast) {
            TargetType::ERREmptyRightHandAssignment(err) => {
                Err(SyntaxError::EmptyRightHandSide(err.get_span()).to_diagnostic(sema.db))
            }
            TargetType::ERRMissingDotInAssignment(err) => {
                Err(SyntaxError::MissingDotInAssignment {
                    file: sema.file,
                    span: err.get_span(),
                }
                .to_diagnostic(sema.db))
            }
            TargetType::ERRMissingEqualInAssignment(err) => {
                Err(SyntaxError::MissingEqualInAssignment {
                    file: sema.file,
                    span: err.get_span(),
                }
                .to_diagnostic(sema.db))
            }
            TargetType::ERROutputAssignInAssignment(err) => {
                Err(SyntaxError::OutputAssignInAssignment {
                    file: sema.file,
                    span: err.get_span(),
                }
                .to_diagnostic(sema.db))
            }
            TargetType::Assignment(assign) => Ok(Stmt::new(
                sema.db,
                StmtKind::Assignment {
                    var,
                    target: assign.children.cast(sema.ast).parse(sema)?,
                },
                self.into(),
                sema.current_scope,
            )),
            TargetType::AssignmentAttempt(attempt) => Ok(Stmt::new(
                sema.db,
                StmtKind::AssignmentAttempt {
                    var,
                    target: attempt.children.cast(sema.ast).parse(sema)?,
                },
                self.into(),
                sema.current_scope,
            )),
        }
    }
}
