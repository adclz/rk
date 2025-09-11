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
    check::{
        check_inheritance::check_methods,
        check_init_expr::check_init_expr,
        errors::{sem_errors::AnalysisError, stmt::StmtError},
    },
    hir_def::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, InitExprKind, Integer, IntegerKind, PathExpr,
                PrimaryExpr, VariableAccessKind,
            },
            spec::{ElementarySpec, Spec, SpecKind},
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
    hir_ty::{
        TyInfo,
        expr_resolver::{ResolvedExpr, ResolvedExprKind},
        init_expr_resolver::{ResolvedInitExpr, resolve_init_expr},
        name_res::pous_in_scope,
        stmt_resolver::{ResolveStmtCtx, ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::{Ty, TyKind, ty_for_pou, ty_for_variable},
        ty_path_expr_resolver::{ResolvePathExprCtx, ResolvedPathElementKind, ResolvedPathResult},
        ty_var_access_resolver::{ResolvedVarKind, ResolvedVarResult},
    },
    to_proto::ToProto,
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
        check_methods(db, ty_for_pou(db, *self), errors);

        if let Some(variables) = match self.pou(db) {
            Pou::Function(f) => Some(f.variables(db)),
            Pou::FunctionBlock(fb) => Some(fb.variables(db)),
            _ => None,
        } {
            for variable in variables {
                let var = ty_for_variable(db, *variable);
                if let Some(init) = variable.init(db) {
                    check_init_expr(db, var, *resolve_init_expr(db, *init), errors);
                }
            }
        }

        if let Some(stmts) = match self.pou(db) {
            Pou::Function(f) => Some(f.statements(db)),
            Pou::FunctionBlock(fb) => Some(fb.statements(db)),
            _ => None,
        } {
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
            ResolvedStmtKind::Assignment { var, target } => {
                if let Err(err) = check_assignment(db, *var, *target) {
                    errors.push(err);
                }
            }
            ResolvedStmtKind::FuncCall { target, params } => {
                if let Ok(ty_var) = target.ty(db) {
                    if let Some(err) = ty_var.as_err(db) {
                        errors.push(err);
                    }
                }
            }
            _ => {
                ctx.finish_block(errors);
            }
        }
    }
}

fn check_assignment<'db>(
    db: &'db dyn BaseDatabase,
    var: ResolvedVarResult<'db>,
    target: ResolvedExpr<'db>,
) -> Result<(), AnalysisError<'db>> {
    let ty_var = var.ty(db)?;

    if var.is_input(db) {
        return Err(AnalysisError::StmtError(StmtError::AssignmentToInput {
            var,
            ty: ty_var,
        }));
    }

    if ty_var.is_callable(db) {
        return Err(AnalysisError::StmtError(StmtError::InvalidAssignment {
            var,
            ty: ty_var,
        }));
    }

    coerce_ty_with_expr(db, ty_var, target)
}

fn coerce_ty_with_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    target_expr: ResolvedExpr<'db>,
) -> Result<(), AnalysisError<'db>> {
    if let TyKind::Target(t) = ty.kind(db) {
        return coerce_ty_with_expr(db, t, target_expr);
    }

    match (ty.kind(db), target_expr.kind(db)) {
        // Assign a simple literal to an elementary expression
        (TyKind::Simple(elem), ResolvedExprKind::Literal(prim)) => elem
            .lit_check(db, *prim)
            .map_err(|err| (err, ty, target_expr).into()),
        (TyKind::Simple(elem), ResolvedExprKind::FuncCall { target, .. }) => {
            let tyy = target.ty(db)?;
            let ret = tyy.has_return_type(db);
            let ret1 = ret.is_some();
            if let Some(ret) = target.ty(db)?.has_return_type(db) {
                coerce_ty_with_ty(db, target_expr, ty, ret)
            } else {
                Err(AnalysisError::StmtError(StmtError::VoidAssignmentTarget {
                    ty,
                    target: *target,
                }))
            }
        }
        (TyKind::Simple(elem), ResolvedExprKind::Bool(lhs, rhs)) => {
            if let ElementarySpec::Bool = elem {
                Ok(())
            } else {
                Err(AnalysisError::StmtError(StmtError::AssignmentIsNotABool {
                    ty,
                    expr: target_expr,
                }))
            }
        }
        _ => todo!(),
    }
}

fn coerce_ty_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    expr: ResolvedExpr<'db>,
    target_ty: Ty<'db>,
    expr_ty: Ty<'db>,
) -> Result<(), AnalysisError<'db>> {
    if let TyKind::Target(t) = target_ty.kind(db) {
        return coerce_ty_with_ty(db, expr, t, expr_ty);
    }

    match (target_ty.kind(db), expr_ty.kind(db)) {
        (TyKind::Simple(elem), TyKind::Simple(elem2)) => {
            if elem == elem2 {
                Ok(())
            } else {
                Err(AnalysisError::StmtError(StmtError::TypeMismatch {
                    expr,
                    ty: target_ty,
                    ty2: expr_ty,
                }))
            }
        }
        _ => todo!(),
    }
}
