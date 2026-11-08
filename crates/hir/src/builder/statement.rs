// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use compact_str::CompactString;

use crate::AstId;
use crate::builder::Parse;
use crate::builder::expression::ParseVariableAccess;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e00_syntax::SyntaxError;
use crate::check::errors::e12_control_flow::ControlFlowError;
use crate::hir_def::expressions::expression::{
    AddOperatorKind, Expr, ExprKind, FuncCall, MultOperatorKind, ParamAssignKind, PrimaryExpr,
    VariableAccess,
};
use crate::hir_def::expressions::statement::{CaseKind, Catch, Stmt, StmtKind};
use crate::hir_def::interned::identifier::SpanIdent;
use auto_lsp::anyhow::{self};
use auto_lsp::core::ast::AstNode;
use auto_lsp::tree_sitter;
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
                                        sema.refuse_after_value(p.chain.as_ref(), &p.value, "the call");
                                        parameters.push(sema.new_param(p.into(), sema.current_scope, kind))
                                    }
                                    ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(p) => {
                                        let value = p.value.cast(sema.ast).parse(sema)?;
                                        let kind = ParamAssignKind::FormalOutput { not: p.not.is_some() , param: SpanIdent::from_node(sema.db, sema, p.param.cast(sema.ast))?, value };
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
                let condition =
                    parse_condition(sema, if_stmt.if_cond.cast(sema.ast), "before the IF")?;
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
                        let condition = parse_condition(
                            sema,
                            else_if_node.else_if_cond.cast(sema.ast),
                            "of its own",
                        )?;
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

                sema.refuse_after_value(
                    for_list.chain.as_ref(),
                    &for_list.initial_value,
                    "the FOR",
                );
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

                let condition =
                    parse_condition(sema, repeat.repeat_cond.cast(sema.ast), "before the UNTIL")?;
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Repeat { body, condition },
                    repeat.into(),
                    sema.current_scope,
                ))
            }
            StmtType::WhileStmt(while_stmt) => {
                let condition = parse_condition(
                    sema,
                    while_stmt.while_cond.cast(sema.ast),
                    "before the WHILE and at the end of its body",
                )?;
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
            StmtType::TryStmt(try_stmt) => {
                type Clause = ast::generated::CatchClause_FinallyClause;
                let body = stmt_list(sema, try_stmt.body.as_ref())?;
                // A __TRY has one __CATCH and one __FINALLY. Another of
                // either is reported, and its statements are read as the end
                // of the first's, which is what the report asks to be
                // written: they are checked like any other. A __CATCH after
                // the __FINALLY is reported and is the one handler there is.
                let mut catch: Option<(Catch<'db>, tree_sitter::Range)> = None;
                let mut finally: Option<(Vec<Stmt<'db>>, tree_sitter::Range)> = None;
                for clause in &try_stmt.clause {
                    let (span, first) = match clause.cast(sema.ast) {
                        Clause::CatchClause(clause) => {
                            let span = clause.get_range().to_owned();
                            let read =
                                catch_clause(sema, clause.variable.as_ref(), clause.body.as_ref())?;
                            match &mut catch {
                                Some((kept, first)) => {
                                    kept.variable = kept.variable.or(read.variable);
                                    kept.body.extend(read.body);
                                    (span, Some(*first))
                                }
                                None => {
                                    if finally.is_some() {
                                        sema.errors.push(
                                            ControlFlowError::CatchAfterFinally { span }
                                                .to_diagnostic(sema.db, sema.file),
                                        );
                                    }
                                    catch = Some((read, span));
                                    (span, None)
                                }
                            }
                        }
                        Clause::FinallyClause(clause) => {
                            let span = clause.get_range().to_owned();
                            let read = stmt_list(sema, clause.body.as_ref())?;
                            match &mut finally {
                                Some((kept, first)) => {
                                    kept.extend(read);
                                    (span, Some(*first))
                                }
                                None => {
                                    finally = Some((read, span));
                                    (span, None)
                                }
                            }
                        }
                    };
                    if let Some(first) = first {
                        sema.errors.push(
                            ControlFlowError::ClauseWrittenTwice { span, first }
                                .to_diagnostic(sema.db, sema.file),
                        );
                    }
                }
                if let Some(closer) = try_stmt.children.as_ref() {
                    let span = closer.cast(sema.ast).get_range().to_owned();
                    sema.errors
                        .push(SyntaxError::EndTrySpelling(span).to_diagnostic(sema.db, sema.file));
                }
                Ok(Stmt::new(
                    sema.db,
                    StmtKind::Try {
                        body,
                        catch: catch.map(|(catch, _)| catch),
                        finally: finally.map(|(finally, _)| finally),
                    },
                    try_stmt.into(),
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
            StmtType::ERRClauseOutsideTry(err) => Err(ControlFlowError::ClauseOutsideTry {
                span: err.get_range().to_owned(),
            }
            .to_diagnostic(sema.db, sema.file)),
        }
    }
}

impl<'db> Parse<'db> for ast::generated::Assign {
    type Output = Stmt<'db>;

    /// `a := b := e`, `x += e`, and a chain of both, `a := b += 1`: the
    /// places in the order written, the value of the last, and for each
    /// place written with an operator its operation. `x += e` is
    /// `x := x + e`: the place is read and written, one access for both, and
    /// the operation is built around it. Neither the reads nor the
    /// operations are in the node index, which holds what the tree has: the
    /// places and the value.
    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Stmt<'db>, IdeDiagnostic> {
        let first = match self.variable.cast(sema.ast) {
            ast::generated::ERRAssignFuncCall_VariableAccess::ERRAssignFuncCall(err) => {
                return Err(
                    SyntaxError::AssignToFunctionCall(err.get_range().to_owned())
                        .to_diagnostic(sema.db, sema.file),
                );
            }
            ast::generated::ERRAssignFuncCall_VariableAccess::VariableAccess(var) => var,
        };
        // Each place with its node, in the order written, and the operator
        // after each but the last with the node it stands in: the link
        // before the next place, its `+=` and that place.
        let mut places = vec![(first.to_access(sema)?, AstId::from(first))];
        let mut operators = Vec::new();
        for link in &self.chain {
            let link = link.cast(sema.ast);
            let operator = link
                .operator
                .as_ref()
                .map(|operator| operator.cast(sema.ast));
            operators.push(operator.map(|operator| (operator, AstId::from(link))));
            let place = link.variable.cast(sema.ast);
            places.push((place.to_access(sema)?, place.into()));
        }
        let read = |sema: &SemanticIndexBuilder<'db>, (var, node): (VariableAccess<'db>, AstId)| {
            Expr::new(
                sema.db,
                ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)),
                node,
                sema.current_scope,
            )
        };
        let last = *places.last().expect("a chain has a place");

        type TargetType = ast::generated::ERREmptyCompoundAssignment_ERREmptyRightHandAssignment_ERRMissingDotInAssignment_ERRMissingEqualInAssignment_ERROutputAssignInAssignment_Assignment_CompoundAssignment;
        let (target, compound) = match self.target.cast(sema.ast) {
            TargetType::ERREmptyRightHandAssignment(err) => {
                return Err(SyntaxError::EmptyRightHandSide(err.get_range().to_owned())
                    .to_diagnostic(sema.db, sema.file));
            }
            TargetType::ERREmptyCompoundAssignment(err) => {
                return Err(SyntaxError::EmptyRightHandSide(err.get_range().to_owned())
                    .to_diagnostic(sema.db, sema.file));
            }
            TargetType::ERRMissingDotInAssignment(err) => {
                return Err(SyntaxError::MissingDotInAssignment {
                    file: sema.file,
                    span: err.get_range().to_owned(),
                }
                .to_diagnostic(sema.db, sema.file));
            }
            TargetType::ERRMissingEqualInAssignment(err) => {
                return Err(SyntaxError::MissingEqualInAssignment {
                    file: sema.file,
                    span: err.get_range().to_owned(),
                }
                .to_diagnostic(sema.db, sema.file));
            }
            TargetType::ERROutputAssignInAssignment(err) => {
                return Err(SyntaxError::OutputAssignInAssignment {
                    file: sema.file,
                    span: err.get_range().to_owned(),
                }
                .to_diagnostic(sema.db, sema.file));
            }
            TargetType::Assignment(assign) => (assign.children.cast(sema.ast).parse(sema)?, false),
            TargetType::CompoundAssignment(assignment) => {
                let value = assignment.value.cast(sema.ast).parse(sema)?;
                let operation =
                    operation(assignment.operator.cast(sema.ast), read(sema, last), value);
                // A statement of one place is its operation. In a chain, the
                // operator and the value mark it.
                let node = match self.chain.is_empty() {
                    true => self.into(),
                    false => assignment.into(),
                };
                (
                    Expr::new(sema.db, operation, node, sema.current_scope),
                    true,
                )
            }
        };
        // `a += b := 1` writes `a` the operation `a + b`, whose right side
        // reads the place after it.
        let operations = operators
            .into_iter()
            .enumerate()
            .map(|(at, operator)| {
                operator.map(|(operator, node)| {
                    let operation =
                        operation(operator, read(sema, places[at]), read(sema, places[at + 1]));
                    Expr::new(sema.db, operation, node, sema.current_scope)
                })
            })
            .collect();
        Ok(Stmt::new(
            sema.db,
            StmtKind::Assignment {
                vars: places.into_iter().map(|(var, _)| var).collect(),
                target,
                compound,
                operations,
            },
            self.into(),
            sema.current_scope,
        ))
    }
}

/// What `+=`, `-=`, `*=` or `/=` writes its place: `x + e` for `x += e`,
/// `x` read on the left.
fn operation<'db>(
    operator: &ast::generated::CompoundOperator,
    left: Expr<'db>,
    right: Expr<'db>,
) -> ExprKind<'db> {
    type Operator = ast::generated::CompoundOperator;
    match operator {
        Operator::Token_PlusEqual(_) => ExprKind::AddOperator {
            left,
            operator: AddOperatorKind::Plus,
            right,
        },
        Operator::Token_MinusEqual(_) => ExprKind::AddOperator {
            left,
            operator: AddOperatorKind::Minus,
            right,
        },
        Operator::Token_StarEqual(_) => ExprKind::MultOperator {
            left,
            operator: MultOperatorKind::Mul,
            right,
        },
        Operator::Token_SlashEqual(_) => ExprKind::MultOperator {
            left,
            operator: MultOperatorKind::Div,
            right,
        },
    }
}

impl<'db> SemanticIndexBuilder<'db> {
    /// What goes on after a value where one assignment stands, in an
    /// argument or at the start of a FOR: a second `:=` (E0032), or a
    /// compound operator (E0033), each with the rest of its chain. The value
    /// is kept, and checked as if nothing followed it.
    pub(crate) fn refuse_after_value(
        &mut self,
        after: Option<
            &auto_lsp::core::ast::AstNodeId<
                ast::generated::ERRChainOutsideStatement_ERRCompoundOutsideStatement,
            >,
        >,
        place: &auto_lsp::core::ast::AstNodeId<ast::generated::Expression>,
        within: &'static str,
    ) {
        let Some(after) = after else {
            return;
        };
        type After = ast::generated::ERRChainOutsideStatement_ERRCompoundOutsideStatement;
        let place_range = place.cast(self.ast).get_range().to_owned();
        let error = match after.cast(self.ast) {
            After::ERRChainOutsideStatement(chain) => SyntaxError::ChainOutsideStatement {
                span: chain.get_range().to_owned(),
                place: place_range,
                within,
            }
            .to_diagnostic(self.db, self.file),
            After::ERRCompoundOutsideStatement(err) => {
                self.compound_outside_statement(err, place_range, within)
            }
        };
        self.errors.push(error);
    }

    /// `x += e` where a value goes on, writing `place` (E0033).
    fn compound_outside_statement(
        &self,
        err: &ast::generated::ERRCompoundOutsideStatement,
        place: tree_sitter::Range,
        within: &'static str,
    ) -> IdeDiagnostic {
        // The value and the rest of the chain after it: `b := 1` in
        // `F(a += b := 1)`.
        let (value, end) = (err.value.cast(self.ast).get_range(), err.get_range());
        SyntaxError::CompoundOutsideStatement {
            operator: err.operator.cast(self.ast).get_range().to_owned(),
            value: tree_sitter::Range {
                start_byte: value.start_byte,
                start_point: value.start_point,
                end_byte: end.end_byte,
                end_point: end.end_point,
            },
            place,
            placement: format!("before {within}"),
            in_condition: false,
        }
        .to_diagnostic(self.db, self.file)
    }
}

/// A condition compares, so `:=` written in one is an assignment where `=`
/// was meant. The grammar keeps both sides, and the left one is the condition
/// the author was writing, so checking carries on from there rather than
/// reporting the shape twice.
///
/// A compound assignment there, `IF x += 1 THEN`, is E0033: dropping its
/// `+` would leave `IF x = 1`, which compiles and means something else.
/// `placement` is where its statement goes instead: before an IF, before a
/// WHILE and at the end of its body, as the condition is checked before
/// each pass, and on its own for an ELSIF, since a statement before the IF
/// would also run when an earlier branch is taken.
fn parse_condition<'db>(
    sema: &mut SemanticIndexBuilder<'db>,
    node: &ast::generated::ERRAssignInCondition_ERRCompoundInCondition_Expression,
    placement: &'static str,
) -> anyhow::Result<Expr<'db>, IdeDiagnostic> {
    type Condition = ast::generated::ERRAssignInCondition_ERRCompoundInCondition_Expression;
    let err = match node {
        Condition::Expression(expr) => {
            return expr.parse(sema);
        }
        Condition::ERRCompoundInCondition(err) => {
            return Err(SyntaxError::CompoundOutsideStatement {
                operator: err.operator.cast(sema.ast).get_range().to_owned(),
                value: err.value.cast(sema.ast).get_range().to_owned(),
                place: err.place.cast(sema.ast).get_range().to_owned(),
                placement: placement.to_string(),
                in_condition: true,
            }
            .to_diagnostic(sema.db, sema.file));
        }
        Condition::ERRAssignInCondition(err) => err,
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

/// The statements of a body, none when it is empty.
fn stmt_list<'db>(
    sema: &mut SemanticIndexBuilder<'db>,
    list: Option<&auto_lsp::core::ast::AstNodeId<ast::generated::StmtList>>,
) -> Result<Vec<Stmt<'db>>, IdeDiagnostic> {
    let Some(list) = list else {
        return Ok(Vec::new());
    };
    list.cast(sema.ast)
        .children
        .iter()
        .map(|stmt| stmt.cast(sema.ast).parse(sema))
        .collect()
}

/// A `__CATCH`, written in its place or after the `__FINALLY`.
fn catch_clause<'db>(
    sema: &mut SemanticIndexBuilder<'db>,
    variable: Option<&auto_lsp::core::ast::AstNodeId<ast::generated::VariableAccess>>,
    body: Option<&auto_lsp::core::ast::AstNodeId<ast::generated::StmtList>>,
) -> Result<Catch<'db>, IdeDiagnostic> {
    let variable = match variable {
        Some(variable) => Some(variable.cast(sema.ast).to_access(sema)?),
        None => None,
    };
    Ok(Catch {
        variable,
        body: stmt_list(sema, body)?,
    })
}
