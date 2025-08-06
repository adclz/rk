use std::ops::Deref;

use crate::check::errors::semantic_errors::{assign_to_function_call, empty_right_hand_assignment};
use crate::hir::expressions::statement::{Stmt, StmtKind};
use crate::parser::expression::{ParseExpression, ParseVariableAccess};
use crate::parser::semantic_index::SemanticIndexBuilder;
use auto_lsp::core::ast::AstNode;
use auto_lsp::{anyhow};

pub trait ParseStatement<'db> {
    fn to_statement(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Stmt<'db>>;
}

impl<'db> ParseStatement<'db> for ast::generated::Stmt {
    fn to_statement(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Stmt<'db>> {
        type StmtType = ast::generated::Stmt;
        match self {
            StmtType::Assign(assign) => assign.to_statement(sema),
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
            _ => todo!(),
        }
    }
}

impl<'db> ParseStatement<'db> for ast::generated::Assign {
    fn to_statement(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Stmt<'db>> {
        let var = match self.variable.deref() {
            ast::generated::ERRAssignFuncCall_Variable::ERRAssignFuncCall(err) => {
                assign_to_function_call(sema.db, err.get_span());
                Err(anyhow::anyhow!("Cannot assign to a function call"))
            }
            ast::generated::ERRAssignFuncCall_Variable::Variable(var) => var.to_access(sema),
        }?;

        match self.target.deref() {
            ast::generated::ERREmptyRightHandAssignment_Assignment_AssignmentAttempt::ERREmptyRightHandAssignment(err) => {
                empty_right_hand_assignment(sema.db, err.get_span());
                Err(anyhow::anyhow!("Empty right-hand side in assignment"))
            },
            ast::generated::ERREmptyRightHandAssignment_Assignment_AssignmentAttempt::Assignment(assign) => Ok(Stmt::new(
                sema.db,
                assign.get_span(),
                StmtKind::Assignment {
                    var,
                    target: assign.children.to_expr(sema)?,
                },
            )),
            ast::generated::ERREmptyRightHandAssignment_Assignment_AssignmentAttempt::AssignmentAttempt(attempt) => {
                unreachable!()
            }
        }
    }
}
