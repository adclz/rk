use compact_str::CompactString;

use crate::builder::Parse;
use crate::builder::expression::ParseVariableAccess;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e00_syntax::SyntaxError;
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
                    StmtKind::FuncCall(FuncCall::new(sema.db, target, parameters)),
                    call.into(),
                    sema.current_scope,
                ))
            }
            StmtType::IfStmt(if_stmt) => {
                let condition = parse_condition(sema, if_stmt.if_cond.cast(sema.ast))?;
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
                        let condition =
                            parse_condition(sema, else_if_node.else_if_cond.cast(sema.ast))?;
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

                if let Some(err) = for_list.children.as_ref() {
                    match err.cast(sema.ast) {
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl_ERROutputAssignInForControl::ERRMissingDotInForControl(err) => {
                            sema.errors.push(SyntaxError::MissingDotInForList {
                                file: sema.file,
                                span: err.get_range().to_owned(),
                            }.to_diagnostic(sema.db, sema.file))
                        }
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl_ERROutputAssignInForControl::ERRMissingEqualInForControl(err) => {
                            sema.errors.push(SyntaxError::MissingEqualInForList {
                                file: sema.file,
                                span: err.get_range().to_owned(),
                            }.to_diagnostic(sema.db, sema.file))
                        }
                        ast::generated::ERRMissingDotInForControl_ERRMissingEqualInForControl_ERROutputAssignInForControl::ERROutputAssignInForControl(err) => {
                            sema.errors.push(SyntaxError::OutputAssignInForList {
                                file: sema.file,
                                span: err.get_range().to_owned(),
                            }.to_diagnostic(sema.db, sema.file))
                        }
                    }
                }

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

                let condition = parse_condition(sema, repeat.repeat_cond.cast(sema.ast))?;
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Repeat { body, condition },
                    repeat.into(),
                    sema.current_scope,
                ))
            }
            StmtType::WhileStmt(while_stmt) => {
                let condition = parse_condition(sema, while_stmt.while_cond.cast(sema.ast))?;
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
            StmtType::RaiseStmt(raise) => {
                let message = raise.message.cast(sema.ast).parse(sema)?;
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Raise { message },
                    raise.into(),
                    sema.current_scope,
                ))
            }
            StmtType::WasmPragma(pragma) => {
                let doc = sema.file.document(sema.db).as_bytes();

                let type_ref = pragma
                    .type_ref
                    .as_ref()
                    .map(|tr| SpanIdent::from_node(sema.db, sema, tr.cast(sema.ast)))
                    .transpose()?;

                let instr_text = pragma
                    .instruction
                    .cast(sema.ast)
                    .get_text(doc)
                    .map_err(|e| {
                        SyntaxError::SyntaxError {
                            span: pragma.instruction.cast(sema.ast).get_range().to_owned(),
                            err: e.to_string(),
                        }
                        .to_diagnostic(sema.db, sema.file)
                    })?;
                let instruction = CompactString::from(&instr_text[1..instr_text.len() - 1]);
                let instruction_span = pragma.instruction.cast(sema.ast).get_range().to_owned();

                let params = pragma.params.as_ref().map_or(Ok(vec![]), |p| {
                    p.cast(sema.ast)
                        .var
                        .iter()
                        .map(|id| SpanIdent::from_node(sema.db, sema, id.cast(sema.ast)))
                        .collect::<Result<Vec<_>, _>>()
                })?;

                let result = pragma
                    .result
                    .as_ref()
                    .map(|r| {
                        SpanIdent::from_node(sema.db, sema, r.cast(sema.ast).var.cast(sema.ast))
                    })
                    .transpose()?;

                Ok(Stmt::new(
                    sema.db,
                    StmtKind::WasmPragma(crate::hir_def::extern_decl::WasmDecl {
                        type_ref,
                        instruction,
                        instruction_span,
                        params,
                        result,
                    }),
                    pragma.into(),
                    sema.current_scope,
                ))
            }
            StmtType::AllowPragma(pragma) => {
                let decl = sema.parse_allow_pragma(pragma);
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::AllowPragma(decl),
                    pragma.into(),
                    sema.current_scope,
                ))
            }
            StmtType::ERRMethodDeclInBody(err) => {
                Err(SyntaxError::MethodDeclInBody(err.get_range().to_owned())
                    .to_diagnostic(sema.db, sema.file))
            }
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
            ast::generated::ERRAssignFuncCall_VariableAccess::ERRAssignFuncCall(err) => Err(
                SyntaxError::AssignToFunctionCall(err.get_range().to_owned())
                    .to_diagnostic(sema.db, sema.file),
            ),
            ast::generated::ERRAssignFuncCall_VariableAccess::VariableAccess(var) => {
                var.to_access(sema)
            }
        }?;

        type TargetType = ast::generated::ERREmptyRightHandAssignment_ERRMissingDotInAssignment_ERRMissingEqualInAssignment_ERROutputAssignInAssignment_Assignment;
        match self.target.cast(sema.ast) {
            TargetType::ERREmptyRightHandAssignment(err) => {
                Err(SyntaxError::EmptyRightHandSide(err.get_range().to_owned())
                    .to_diagnostic(sema.db, sema.file))
            }
            TargetType::ERRMissingDotInAssignment(err) => {
                Err(SyntaxError::MissingDotInAssignment {
                    file: sema.file,
                    span: err.get_range().to_owned(),
                }
                .to_diagnostic(sema.db, sema.file))
            }
            TargetType::ERRMissingEqualInAssignment(err) => {
                Err(SyntaxError::MissingEqualInAssignment {
                    file: sema.file,
                    span: err.get_range().to_owned(),
                }
                .to_diagnostic(sema.db, sema.file))
            }
            TargetType::ERROutputAssignInAssignment(err) => {
                Err(SyntaxError::OutputAssignInAssignment {
                    file: sema.file,
                    span: err.get_range().to_owned(),
                }
                .to_diagnostic(sema.db, sema.file))
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
        }
    }
}

/// A condition compares, so `:=` written in one is an assignment where `=`
/// was meant. The grammar keeps both sides, and the left one is the condition
/// the author was writing, so checking carries on from there rather than
/// reporting the shape twice.
fn parse_condition<'db>(
    sema: &mut SemanticIndexBuilder<'db>,
    node: &ast::generated::ERRAssignInCondition_Expression,
) -> anyhow::Result<crate::hir_def::expressions::expression::Expr<'db>, IdeDiagnostic> {
    let err = match node {
        ast::generated::ERRAssignInCondition_Expression::Expression(expr) => {
            return expr.parse(sema);
        }
        ast::generated::ERRAssignInCondition_Expression::ERRAssignInCondition(err) => err,
    };

    // Between the two sides is the `:=` and whatever spaces surround it,
    // which is all a fix should touch: replacing the whole condition would
    // take both operands with it.
    let span = err.get_range().to_owned();
    let sign = match (err.children.first(), err.children.get(1)) {
        (Some(left), Some(right)) => {
            let left = left.cast(sema.ast).get_range().to_owned();
            let right = right.cast(sema.ast).get_range().to_owned();
            auto_lsp::tree_sitter::Range {
                start_byte: left.end_byte,
                end_byte: right.start_byte,
                start_point: left.end_point,
                end_point: right.start_point,
            }
        }
        _ => span,
    };

    Err(SyntaxError::AssignInCondition {
        file: sema.file,
        span,
        sign,
    }
    .to_diagnostic(sema.db, sema.file))
}
