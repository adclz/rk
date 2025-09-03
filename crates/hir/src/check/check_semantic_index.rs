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
    check::errors::sem_errors::{AnalysisError, PathExprError, StmtError},
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
        semantic_index::{semantic_index, HirNode, SemanticIndex},
    },
    to_proto::ToProto,
    ty::{
        expr_resolver::{ResolvedExpr, ResolvedExprKind}, name_res::pous_in_scope, stmt_resolver::{resolve_stmt, ResolveStmtCtx, ResolvedStmt, ResolvedStmtKind}, ty::{ty_for_pou, ty_for_variable, Ty, TyKind}, ty_path_expr_resolver::{ResolvePathExprCtx, ResolvedPathElementKind, ResolvedPathResult}, ty_var_access_resolver::ResolvedVarKind, TyInfo
    },
    walk::WalkHir,
};

pub trait Check<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>);
}

impl<'db> Check<'db> for SemanticIndex<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
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

    fn mark_and_check(&mut self, stmt: ResolvedStmt<'db>) {
        if self.return_reached.is_some() {
            match &mut self.unreachable_range {
                Some((_, end)) => {
                    *end = stmt.get_span(self.db).clone();
                }
                None => {
                    let span = stmt.get_span(self.db).clone();
                    self.unreachable_range = Some((span.clone(), span));
                }
            }
        }

        match stmt.kind(self.db) {
            ResolvedStmtKind::Continue => self.continue_reached = Some(stmt),
            ResolvedStmtKind::Exit => self.exit_reached = Some(stmt),
            ResolvedStmtKind::Return { .. } => self.return_reached = Some(stmt),
            _ => {}
        }
    }

    fn finish_block(&mut self, errors: &mut Vec<AnalysisError<'db>>) {
        if let Some((start, end)) = self.unreachable_range.take() {
            errors.push(AnalysisError::StmtError(StmtError::Unreachable {
                start,
                end,
            }));
        }
    }
}

impl<'db> Check<'db> for PouDecl<'db> {
    fn collect_errors(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        if let Some(stmts) = self.get_stmts(db) {
            let mut stmt_ctx = StmtCheckCtx::new(db);

            for stmt in stmts {
                resolve_stmt(db, *stmt).collect_errors_with_ctx(db, &mut stmt_ctx, errors);
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

impl<'db> CheckWithCtx<'db> for Vec<ResolvedStmt<'db>> {
    fn collect_errors_with_ctx(
        &self,
        db: &'db dyn BaseDatabase,
        ctx: &mut StmtCheckCtx<'db>,
        errors: &mut Vec<AnalysisError<'db>>,
    ) {
        self.iter().for_each(|s| {
            s.collect_errors_with_ctx(db, ctx, errors);
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
        ctx.mark_and_check(*self);

        match self.kind(db) {
            ResolvedStmtKind::For { body, .. } => {
                body.collect_errors_with_ctx(db, ctx, errors);
                ctx.finish_block(errors);
            }
            ResolvedStmtKind::While { body, .. } => {
                body.collect_errors_with_ctx(db, ctx, errors);
                ctx.finish_block(errors);
            }
            ResolvedStmtKind::Repeat { body, .. } => {
                body.collect_errors_with_ctx(db, ctx, errors);
                ctx.finish_block(errors);
            }
            ResolvedStmtKind::If {
                then,
                else_,
                else_if,
                ..
            } => {
                then.collect_errors_with_ctx(db, ctx, errors);
                ctx.finish_block(errors);

                else_.collect_errors_with_ctx(db, ctx, errors);
                ctx.finish_block(errors);

                else_if
                    .iter()
                    .for_each(|(_, s)| s.collect_errors_with_ctx(db, ctx, errors));
                ctx.finish_block(errors);
            }
            ResolvedStmtKind::Assignment { var, target } => match var.ty(db) {
                Some(ty_var) => {
                    if ty_var.is_callable(db) {
                        errors.push(AnalysisError::StmtError(StmtError::AssignmentToCallable {
                            loc: self.get_span(db),
                            ty: ty_var,
                        }));
                    } else if let Some(err) = ty_var.as_err(db) {
                        errors.push(err);
                    } else if let Err(err) = coerce_ty_expr(db, ty_var, *target) {
                        errors.push(err);
                    }
                }
                None => if let Some(err) = var.is_err(db) {
                    errors.push(err);
                }
            },
            ResolvedStmtKind::FuncCall { target, params } => match target.ty(db) {
                Some(ty_var) => {
                    if let Some(err) = ty_var.as_err(db) {
                        errors.push(err);
                    } else if !ty_var.is_callable(db) {
                    }
                }
                None => {}
            },
            _ => {
                ctx.finish_block(errors);
            }
        }
    }
}

fn coerce_ty_expr<'db>(
    db: &'db dyn BaseDatabase,
    target_ty: Ty<'db>,
    expr: ResolvedExpr<'db>,
) -> Result<(), AnalysisError<'db>> {
    match (target_ty.kind(db), expr.kind(db)) {
        // Assign a simple literal to an elementary expression
        (TyKind::Simple(elem), ResolvedExprKind::Literal(prim)) => elem
            .lit_check(db, *prim)
            .map_err(|err| (err, target_ty, expr).into()),
        _ => todo!(),
    }
}
