use std::sync::Arc;

use crate::check::errors::semantic_errors::{assign_direct_pou_to_a_variable, continue_outside_loop, exit_outside_loop, unreachable_code};
use crate::hir::expressions::statement::{Stmt, StmtKind};
use crate::hir::scope::FileScopeId;
use crate::hir_ty::expr_resolver::{resolve_expr, Env, ResolvedExpr};
use crate::hir_ty::ty::{Ty, TyKind};
use crate::hir_ty::ty_var_access_resolver::{resolve_var_access, ResolvedVarResult};
use crate::hir_ty::TyResolved;
use crate::to_proto::ToProto;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(no_eq)]
pub fn resolve_stmts<'db>(
    db: &'db dyn BaseDatabase,
    stmts: &'db [Stmt<'db>],
    scope_id: FileScopeId,
) -> Arc<ResolveStmtsResult<'db>> {
    Arc::new(ResolveStmtCtx::new(db, stmts, None, scope_id).resolve())
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedStmt<'db> {
    pub kind: ResolvedStmtKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedStmtKind<'db> {
    Assignment {
        var: Arc<ResolvedVarResult<'db>>,
        target: Arc<ResolvedExpr<'db>>,
    },
    AssignmentAttempt {
        var: Arc<ResolvedVarResult<'db>>,
        target: Arc<ResolvedExpr<'db>>,
    },
    Invocation {},
    FuncCall {},
    If {
        condition: Arc<ResolvedExpr<'db>>,
        then: Option<Arc<ResolveStmtsResult<'db>>>,
        else_if: Vec<(Arc<ResolvedExpr<'db>>, Arc<ResolveStmtsResult<'db>>)>,
        else_: Option<Arc<ResolveStmtsResult<'db>>>,
    },
    Case {},
    For {
        control_var: Arc<ResolvedVarResult<'db>>,
        start: Arc<ResolvedExpr<'db>>,
        end: Arc<ResolvedExpr<'db>>,
        step: Option<Arc<ResolvedExpr<'db>>>,
        body: Arc<ResolveStmtsResult<'db>>,
    },
    Repeat {
        condition: Arc<ResolvedExpr<'db>>,
        body: Arc<ResolveStmtsResult<'db>>,
    },
    While {
        condition: Arc<ResolvedExpr<'db>>,
        body: Arc<ResolveStmtsResult<'db>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolveStmtsResult<'db> {
    pub stmts: Vec<ResolvedStmt<'db>>,
    pub errors: Vec<StmtResolveError<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtResolveError<'db> {
    ContinueOutsideLoop { continue_stmt: Stmt<'db> },
    ExitOutsideLoop { exit_stmt: Stmt<'db> },
    Unreachable { start: Span, end: Span },
    AssignmentToCallable { loc: Span, ty: Ty<'db> },
}

impl<'db> StmtResolveError<'db> {
    pub fn emit_error(&self, db: &'db dyn BaseDatabase) {
        match self {
            StmtResolveError::ContinueOutsideLoop { continue_stmt } => {
                continue_outside_loop(db, *continue_stmt);
            }
            StmtResolveError::ExitOutsideLoop { exit_stmt } => {
                exit_outside_loop(db, *exit_stmt);
            }
            StmtResolveError::Unreachable { start, end } => {
                unreachable_code(db, start, end); 
            }
            StmtResolveError::AssignmentToCallable { loc, ty } => {
                assign_direct_pou_to_a_variable(db, loc, ty.origin(db));
            }
        }
    }
}

pub struct ResolveStmtCtx<'db> {
    db: &'db dyn BaseDatabase,
    // The statements to resolve
    stmt: &'db [Stmt<'db>],
    scope_id: FileScopeId,
    // Solved statements
    resolved: Vec<ResolvedStmt<'db>>,
    // Errors encountered during resolution
    errors: Vec<StmtResolveError<'db>>,
    // If a continue statement was reached
    continue_reached: Option<Stmt<'db>>,
    // If an exit statement was reached
    exit_reached: Option<Stmt<'db>>,
    // If a return statement was reached
    return_reached: Option<Stmt<'db>>,
    unreachable_range: Option<(Span, Span)>,
    // Are we inside a statement context?
    stmt_ctx: Option<Stmt<'db>>,
}

impl<'db> ResolveStmtCtx<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        stmt: &'db [Stmt<'db>],
        ctx: Option<Stmt<'db>>,
        scope_id: FileScopeId,
    ) -> Self {
        Self {
            db,
            stmt,
            scope_id,
            stmt_ctx: ctx,
            resolved: vec![],
            errors: vec![],
            continue_reached: None,
            exit_reached: None,
            return_reached: None,
            unreachable_range: None,
        }
    }

    pub fn resolve(mut self) -> ResolveStmtsResult<'db> {
        self.stmt.iter().for_each(|stmt| {
            // Checks if the statement is reachable
            if let Some(ret) = self.return_reached {
                match &mut self.unreachable_range {
                    None => {
                        self.unreachable_range = Some((stmt.get_span(self.db).clone(), stmt.get_span(self.db).clone()));
                    }
                    Some((start, end)) => {
                        self.unreachable_range = Some((start.clone(), stmt.get_span(self.db).clone()));
                    }
                }
            }

            match stmt.stmt(self.db) {
                StmtKind::Assignment { var, target } => {
                    let resolved_var = resolve_var_access(self.db, self.scope_id, var);
                    if let Some(ty) = resolved_var.ty() {
                        if let TyKind::Callable { .. } = ty.kind(self.db) {
                            self.errors.push(StmtResolveError::AssignmentToCallable { loc: resolved_var.origin.get_span(self.db).clone(), ty });
                            return;
                        }

                        let resolved_target = resolve_expr(self.db, Env::Ty(ty), *target);
                        self.resolved.push(ResolvedStmt {
                            kind: ResolvedStmtKind::Assignment {
                                var: resolved_var,
                                target: resolved_target,
                            },
                        })
                    }
                }
                StmtKind::AssignmentAttempt { var, target } => {
                    let resolved_var = resolve_var_access(self.db, self.scope_id, var);
                    if let Some(ty) = resolved_var.ty() {
                        let resolved_target = resolve_expr(self.db, Env::Ty(ty), *target);
                        self.resolved.push(ResolvedStmt {
                            kind: ResolvedStmtKind::AssignmentAttempt {
                                var: resolved_var,
                                target: resolved_target,
                            },
                        });
                    }
                }
                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    self.resolved.push(ResolvedStmt {
                        kind: ResolvedStmtKind::If {
                            condition: resolve_expr(self.db, Env::Bool, *condition),
                            then: then
                                .as_ref()
                                .map(|then| resolve_stmts(self.db, &then, self.scope_id)),
                            else_if: else_if
                                .into_iter()
                                .map(|(cond, stmts)| {
                                    (
                                        resolve_expr(self.db, Env::Bool, *cond),
                                        resolve_stmts(self.db, &stmts, self.scope_id),
                                    )
                                })
                                .collect(),
                            else_: else_
                                .as_ref()
                                .map(|else_| resolve_stmts(self.db, &else_, self.scope_id)),
                        },
                    });
                }
                StmtKind::Case {
                    condition,
                    cases,
                    else_,
                } => {
                    self.resolved.push(ResolvedStmt {
                        kind: ResolvedStmtKind::Case {},
                    });
                }
                StmtKind::Invocation { target, params } => {
                    self.resolved.push(ResolvedStmt {
                        kind: ResolvedStmtKind::Invocation {},
                    });
                }
                StmtKind::FuncCall { target, params } => {
                    self.resolved.push(ResolvedStmt {
                        kind: ResolvedStmtKind::FuncCall {},
                    });
                }
                StmtKind::For {
                    control_variable,
                    start,
                    end,
                    step,
                    body,
                } => {
                    let control_var = resolve_var_access(self.db, self.scope_id, control_variable);
                    if let Some(ty) = control_var.ty() {
                        let start_expr = resolve_expr(self.db, Env::Ty(ty), *start);
                        let end_expr = resolve_expr(self.db, Env::Ty(ty), *end);
                        let step_expr = step
                            .as_ref()
                            .map(|s| resolve_expr(self.db, Env::Ty(ty), *s));
                        self.resolved.push(ResolvedStmt {
                            kind: ResolvedStmtKind::For {
                                control_var,
                                start: start_expr,
                                end: end_expr,
                                step: step_expr,
                                body: resolve_stmts(self.db, &body, self.scope_id),
                            },
                        });
                    }
                }
                StmtKind::Repeat { body, condition } => {
                    self.resolved.push(ResolvedStmt {
                        kind: ResolvedStmtKind::Repeat {
                            condition: resolve_expr(self.db, Env::Bool, *condition),
                            body: resolve_stmts(self.db, &body, self.scope_id),
                        },
                    });
                }
                StmtKind::While { condition, body } => {
                    self.resolved.push(ResolvedStmt {
                        kind: ResolvedStmtKind::While {
                            condition: resolve_expr(self.db, Env::Bool, *condition),
                            body: resolve_stmts(self.db, &body, self.scope_id),
                        },
                    });
                }
                // "If the EXIT or CONTINUE statement (feature 9 or 11) is supported,
                // then it shall be supported for all of the iteration statements (FOR, WHILE, REPEAT)
                // which are supported in the implementation".

                // CONTINUE breaks the current iteration of the loop and continues with the next iteration.
                StmtKind::Continue => match self.stmt_ctx {
                    Some(ctx)
                        if matches!(
                            ctx.stmt(self.db),
                            StmtKind::For { .. } | StmtKind::Repeat { .. } | StmtKind::While { .. }
                        ) =>
                    {
                        self.continue_reached = Some(*stmt);
                    }
                    _ => {
                        self.errors.push(StmtResolveError::ContinueOutsideLoop {
                            continue_stmt: *stmt,
                        });
                    }
                },
                // EXIT breaks the loop and continues with the next statement after the loop.
                StmtKind::Exit => match self.stmt_ctx {
                    Some(ctx)
                        if matches!(
                            ctx.stmt(self.db),
                            StmtKind::For { .. } | StmtKind::Repeat { .. } | StmtKind::While { .. }
                        ) =>
                    {
                        self.exit_reached = Some(*stmt);
                    }
                    _ => {
                        self.errors
                            .push(StmtResolveError::ExitOutsideLoop { exit_stmt: *stmt });
                    }
                },
                // RETURNS stops the execution of all statements in the current context
                // and returns to the caller.
                StmtKind::Return => {
                    self.return_reached = Some(*stmt);
                }
                StmtKind::Super => {}
            }
        });

        if let Some((start, end)) = self.unreachable_range {
            self.errors.push(StmtResolveError::Unreachable { start, end });
        }

        self.errors.iter().for_each(|error| {
            error.emit_error(self.db);
        });

        ResolveStmtsResult {
            stmts: self.resolved,
            errors: self.errors,
        }
    }
}
