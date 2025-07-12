use std::ops::Deref;

use crate::hir::expression::Variable;
use crate::hir::expression::{
    AccessOperator, MultiElemVarElement, SizeOperator,
    SymbolicVariableKind, VarAccess,
};
use crate::hir::statement::{Stmt, StmtKind};
use crate::ident::Ident;
use crate::parser::expression::{ParseExpression, ParseVariableAccess};
use auto_lsp::core::ast::AstNode;
use auto_lsp::{anyhow, default::db::file::File};

pub trait ParseStatement<'db> {
    fn to_statement(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Stmt<'db>>;
}

impl<'db> ParseStatement<'db> for ast::generated::Stmt {
    fn to_statement(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Stmt<'db>> {
        type StmtType = ast::generated::Stmt;
        match self {
            StmtType::Assign(assign) => assign.to_statement(db, file),
            StmtType::SuperStmt(super_stmt) => {
                Ok(Stmt::new(db, super_stmt.get_span(), StmtKind::Super))
            }
            StmtType::Token_RETURN(return_stmt) => {
                Ok(Stmt::new(db, return_stmt.get_span(), StmtKind::Return))
            }
            StmtType::Token_CONTINUE(stmt) => {
                Ok(Stmt::new(db, stmt.get_span(), StmtKind::Continue))
            }
            StmtType::Token_EXIT(stmt) => Ok(Stmt::new(db, stmt.get_span(), StmtKind::Exit)),
            _ => todo!(),
        }
    }
}

impl<'db> ParseStatement<'db> for ast::generated::Assign {
    fn to_statement(
        &'db self,
        db: &'db dyn auto_lsp::default::db::BaseDatabase,
        file: File,
    ) -> anyhow::Result<Stmt<'db>> {
        let var = self.variable.to_access(db, file)?;

        match self.target.deref() {
            ast::generated::Assignment_AssignmentAttempt_RefAssign::Assignment(assign) => {
                Ok(Stmt::new(
                    db,
                    assign.get_span(),
                    StmtKind::Assignment {
                        var,
                        target: assign.children.to_expr(db, file)?,
                    },
                ))
            }
            ast::generated::Assignment_AssignmentAttempt_RefAssign::AssignmentAttempt(attempt) => {
                unreachable!()
            }
            ast::generated::Assignment_AssignmentAttempt_RefAssign::RefAssign(ref_assign) => {
                unreachable!()
            }
        }
    }
}
