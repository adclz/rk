#![allow(unused_imports)]
#![allow(dead_code)]
use std::{error::Error, fmt::format, num::ParseIntError, str::ParseBoolError};

use ast::generated::ConstantExpr;
use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase, file::File},
    lsp_types::DiagnosticRelatedInformation,
};
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    check::errors::sem_errors::AnalysisError,
    def::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, InitExprKind, Integer, IntegerKind, PathExpr,
                PrimaryExpr, VariableAccessKind,
            },
            spec::{Spec, SpecKind},
            statement::{Stmt, StmtKind},
        },
        interned::namespace::NamespacePath,
        namespace::NamespaceDecl,
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::FileScopeId,
        semantic_index::{HirNode, SemanticIndex, semantic_index},
    },
    to_proto::ToProto,
    ty::{
        expr_resolver::ResolvedExpr,
        name_res::pous_in_scope,
        stmt_resolver::{
            ResolveStmtCtx, ResolveStmtsResult, ResolvedStmt, ResolvedStmtKind, resolve_stmt,
        },
        ty::{Ty, TyKind, ty_for_pou, ty_for_variable},
        ty_path_expr_resolver::ResolvePathExprCtx,
    },
    walk::WalkHir,
};

pub trait Check<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>);
}

impl<'db> Check<'db> for SemanticIndex<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        eprintln!("global pous: {}", self.global_pous.len());
        eprintln!("namespaces: {}", self.namespaces.len());
        self.errors.iter().for_each(|err| errors.push(err.clone()));
        self.global_pous
            .iter()
            .for_each(|p| p.collect_errors(db, errors));
        self.namespaces
            .iter()
            .for_each(|n| n.collect_errors(db, errors));
    }
}

impl<'db> Check<'db> for NamespaceDecl<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        self.pous(db)
            .iter()
            .for_each(|p| p.collect_errors(db, errors));
    }
}

struct StmtCheckCtx<'db> {
    db: &'db dyn BaseDatabase,
    continue_reached: Option<ResolvedStmt<'db>>,
    exit_reached: Option<ResolvedStmt<'db>>,
    return_reached: Option<ResolvedStmt<'db>>,
    unreachable_range: Option<(Span, Span)>,
}

impl<'db> StmtCheckCtx<'db> {
    fn new(db: &'db dyn BaseDatabase) -> Self {
        Self {
            db,
            continue_reached: None,
            exit_reached: None,
            return_reached: None,
            unreachable_range: None,
        }
    }

    fn mark(&mut self, stmt: ResolvedStmt<'db>) {
        match stmt.kind(self.db) {
            // "If the EXIT or CONTINUE statement (feature 9 or 11) is supported,
            // then it shall be supported for all of the iteration statements (FOR, WHILE, REPEAT)
            // which are supported in the implementation".

            // CONTINUE breaks the current iteration of the loop and continues with the next iteration.
            ResolvedStmtKind::Continue => {
                self.continue_reached = Some(stmt);
            }
            // EXIT breaks the loop and continues with the next statement after the loop.
            ResolvedStmtKind::Exit => {
                self.exit_reached = Some(stmt);
            }
            // RETURNS stops the execution of all statements in the current context
            // and returns to the caller.
            ResolvedStmtKind::Return { .. } => {
                self.return_reached = Some(stmt);
            }
            _ => {}
        }
    }

    fn update_unreachable(&mut self, stmt: ResolvedStmt<'db>) {
        if let Some(ret) = self.return_reached {
            match &mut self.unreachable_range {
                None => {
                    self.unreachable_range = Some((
                        stmt.get_span(self.db).clone(),
                        stmt.get_span(self.db).clone(),
                    ));
                }
                Some((start, end)) => {
                    self.unreachable_range = Some((start.clone(), stmt.get_span(self.db).clone()));
                }
            }
        }
    }
}

impl<'db> Check<'db> for PouDecl<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        if let Some(stmts) = self.get_stmts(db) {
            let mut stmt_ctx = StmtCheckCtx::new(db);

            for stmt in stmts {
                resolve_stmt(db, *stmt, self.scope_id(db))
                    .stmt(db)
                    .map(|s| s.collect_errors_with_ctx(db, &mut stmt_ctx, errors));
            }
        }
    }
}

trait CheckWithCtx<'db> {
    fn collect_errors_with_ctx(
        &self,
        db: &'db dyn BaseDatabase,
        ctx: &mut StmtCheckCtx<'db>,
        errors: &mut Vec<AnalysisError<'db>>,
    );
}

impl<'db> CheckWithCtx<'db> for Vec<ResolveStmtsResult<'db>> {
    fn collect_errors_with_ctx(
        &self,
        db: &'db dyn BaseDatabase,
        ctx: &mut StmtCheckCtx<'db>,
        errors: &mut Vec<AnalysisError<'db>>,
    ) {
        self.iter().for_each(|s| {
            s.stmt(db)
                .map(|s| s.collect_errors_with_ctx(db, ctx, errors));
        })
    }
}

impl<'db> CheckWithCtx<'db> for ResolvedStmt<'db> {
    fn collect_errors_with_ctx(
        &self,
        db: &'db dyn BaseDatabase,
        ctx: &mut StmtCheckCtx<'db>,
        errors: &mut Vec<AnalysisError<'db>>,
    ) {
        ctx.mark(*self);
        match self.kind(db) {
            ResolvedStmtKind::For { body, .. } => {
                body.collect_errors_with_ctx(db, ctx, errors);
            }
            ResolvedStmtKind::While { body, .. } => {
                body.collect_errors_with_ctx(db, ctx, errors);
            }
            ResolvedStmtKind::Repeat { body, .. } => {
                body.collect_errors_with_ctx(db, ctx, errors);
            }
            ResolvedStmtKind::If {
                then,
                else_,
                else_if,
                ..
            } => {
                then.collect_errors_with_ctx(db, ctx, errors);
                else_.collect_errors_with_ctx(db, ctx, errors);
                else_if
                    .iter()
                    .for_each(|(_, s)| s.collect_errors_with_ctx(db, ctx, errors));
            }
            _ => {}
        }
    }
}
