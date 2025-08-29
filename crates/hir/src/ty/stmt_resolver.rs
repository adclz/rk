use crate::check::errors::sem_errors::StmtError;
use crate::def::expressions::statement::{Stmt, StmtKind};
use crate::def::scope::FileScopeId;
use crate::to_proto::{AstId, ToProto};
use crate::ty::TyResolved;
use crate::ty::expr_resolver::{Env, ResolvedExpr, ResolvedExprKind, resolve_expr};
use crate::ty::ty::{TyKind};
use crate::ty::ty_var_access_resolver::{ResolvedVarResult, resolve_var_access};
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolve_stmt<'db>(
    db: &'db dyn BaseDatabase,
    stmt: Stmt<'db>,
    scope_id: FileScopeId,
) -> ResolveStmtsResult<'db> {
    ResolveStmtCtx::new(db, stmt, scope_id).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolveStmtsResult<'db> {
    #[tracked]
    #[returns(ref)]
    #[no_eq]
    pub stmt: Option<ResolvedStmt<'db>>,
    #[tracked]
    #[returns(ref)]
    #[no_eq]
    pub errors: Vec<StmtError<'db>>,
}

#[salsa::tracked(debug)]
pub struct ResolvedStmt<'db> {
    pub id: AstId,
    pub scope_id: FileScopeId,
    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub kind: ResolvedStmtKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedStmtKind<'db> {
    Assignment {
        var: ResolvedVarResult<'db>,
        target: ResolvedExpr<'db>,
    },
    AssignmentAttempt {
        var: ResolvedVarResult<'db>,
        target: ResolvedExpr<'db>,
    },
    Invocation {},
    FuncCall {},
    If {
        condition: ResolvedExpr<'db>,
        then: Vec<ResolveStmtsResult<'db>>,
        else_if: Vec<(ResolvedExpr<'db>, Vec<ResolveStmtsResult<'db>>)>,
        else_: Vec<ResolveStmtsResult<'db>>,
    },
    Case {},
    For {
        control_var: ResolvedVarResult<'db>,
        start: ResolvedExpr<'db>,
        end: ResolvedExpr<'db>,
        step: Option<ResolvedExpr<'db>>,
        body: Vec<ResolveStmtsResult<'db>>,
    },
    Repeat {
        condition: ResolvedExpr<'db>,
        body: Vec<ResolveStmtsResult<'db>>,
    },
    While {
        condition: ResolvedExpr<'db>,
        body: Vec<ResolveStmtsResult<'db>>,
    },
    Continue,
    Exit,
    Return,
    Super,
}

pub struct ResolveStmtCtx<'db> {
    db: &'db dyn BaseDatabase,
    // The statements to resolve
    stmt: Stmt<'db>,
    scope_id: FileScopeId,
    // Solved statements
    resolved: Option<ResolvedStmt<'db>>,
    // Errors encountered during resolution
    errors: Vec<StmtError<'db>>,
}

impl<'db> ResolveStmtCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, stmt: Stmt<'db>, scope_id: FileScopeId) -> Self {
        Self {
            db,
            stmt,
            scope_id,
            resolved: None,
            errors: vec![],
        }
    }

    pub fn resolve(mut self) -> ResolveStmtsResult<'db> {
        let stmt = self.stmt;
        match stmt.stmt(self.db) {
            StmtKind::Assignment { var, target } => {
                let resolved_var = resolve_var_access(self.db, self.scope_id, var);

                if let Some(ty) = resolved_var.ty(self.db) {
                    let resolved_target = resolve_expr(self.db, Env::Ty(ty), *target);

                    match ty.kind(self.db) {
                        TyKind::Function { .. } => {
                            self.errors.push(StmtError::AssignmentToCallable {
                                loc: resolved_var.origin(self.db).get_span(self.db).clone(),
                                ty,
                            });
                        }
                        TyKind::Simple(elementary) => {
                            if let ResolvedExprKind::Literal(elem) = resolved_target.kind(self.db) {
                                if let Err(err) = elementary.lit_check(self.db, *elem) {}
                            }
                        }
                        _ => {}
                    }

                    self.resolved = Some(ResolvedStmt::new(
                        self.db,
                        stmt.id(self.db),
                        stmt.scope_id(self.db),
                        ResolvedStmtKind::Assignment {
                            var: *resolved_var,
                            target: *resolved_target,
                        },
                    ))
                }
            }
            StmtKind::AssignmentAttempt { var, target } => {
                let resolved_var = resolve_var_access(self.db, self.scope_id, var);
                if let Some(ty) = resolved_var.ty(self.db) {
                    let resolved_target = resolve_expr(self.db, Env::Ty(ty), *target);
                    self.resolved = Some(ResolvedStmt::new(
                        self.db,
                        stmt.id(self.db),
                        stmt.scope_id(self.db),
                        ResolvedStmtKind::AssignmentAttempt {
                            var: *resolved_var,
                            target: *resolved_target,
                        },
                    ));
                }
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => self.resolved = Some(ResolvedStmt::new(
                self.db,
                stmt.id(self.db),
                stmt.scope_id(self.db),
                ResolvedStmtKind::If {
                    condition: *resolve_expr(self.db, Env::Bool, *condition),
                    then: then
                        .as_ref()
                        .map(|then| {
                            then.iter()
                                .map(|s| *resolve_stmt(self.db, *s, self.scope_id))
                                .collect()
                        })
                        .unwrap_or_default(),
                    else_if: else_if
                        .iter()
                        .map(|(cond, stmts)| {
                            (
                                *resolve_expr(self.db, Env::Bool, *cond),
                                stmts
                                    .iter()
                                    .map(|s| *resolve_stmt(self.db, *s, self.scope_id))
                                    .collect(),
                            )
                        })
                        .collect(),
                    else_: else_
                        .as_ref()
                        .map(|else_| {
                            else_
                                .iter()
                                .map(|s| *resolve_stmt(self.db, *s, self.scope_id))
                                .collect()
                        })
                        .unwrap_or_default(),
                },
            )),
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                self.resolved = Some(ResolvedStmt::new(
                    self.db,
                    stmt.id(self.db),
                    stmt.scope_id(self.db),
                    ResolvedStmtKind::Case {},
                ));
            }
            StmtKind::Invocation { target, params } => {
                self.resolved = Some(ResolvedStmt::new(
                    self.db,
                    stmt.id(self.db),
                    stmt.scope_id(self.db),
                    ResolvedStmtKind::Invocation {},
                ));
            }
            StmtKind::FuncCall { target, params } => {
                self.resolved = Some(ResolvedStmt::new(
                    self.db,
                    stmt.id(self.db),
                    stmt.scope_id(self.db),
                    ResolvedStmtKind::FuncCall {},
                ));
            }
            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body,
            } => {
                let control_var = resolve_var_access(self.db, self.scope_id, control_variable);
                if let Some(ty) = control_var.ty(self.db) {
                    let start_expr = resolve_expr(self.db, Env::Ty(ty), *start);
                    let end_expr = resolve_expr(self.db, Env::Ty(ty), *end);
                    let step_expr = step
                        .as_ref()
                        .map(|s| resolve_expr(self.db, Env::Ty(ty), *s));
                    self.resolved = Some(ResolvedStmt::new(
                        self.db,
                        stmt.id(self.db),
                        stmt.scope_id(self.db),
                        ResolvedStmtKind::For {
                            control_var: *control_var,
                            start: *start_expr,
                            end: *end_expr,
                            step: step_expr.copied(),
                            body: body
                                .iter()
                                .map(|s| *resolve_stmt(self.db, *s, self.scope_id))
                                .collect(),
                        },
                    ));
                }
            }
            StmtKind::Repeat { body, condition } => {
                self.resolved = Some(ResolvedStmt::new(
                    self.db,
                    stmt.id(self.db),
                    stmt.scope_id(self.db),
                    ResolvedStmtKind::Repeat {
                        condition: *resolve_expr(self.db, Env::Bool, *condition),
                        body: body
                            .iter()
                            .map(|s| *resolve_stmt(self.db, *s, self.scope_id))
                            .collect(),
                    },
                ));
            }
            StmtKind::While { condition, body } => {
                self.resolved = Some(ResolvedStmt::new(
                    self.db,
                    stmt.id(self.db),
                    stmt.scope_id(self.db),
                    ResolvedStmtKind::While {
                        condition: *resolve_expr(self.db, Env::Bool, *condition),
                        body: body
                            .iter()
                            .map(|s| *resolve_stmt(self.db, *s, self.scope_id))
                            .collect(),
                    },
                ));
            } StmtKind::Continue => self.resolved = Some(ResolvedStmt::new(
                self.db,
                stmt.id(self.db),
                stmt.get_scope_id(self.db),
                ResolvedStmtKind::Continue,
            )),
            StmtKind::Exit => self.resolved = Some(ResolvedStmt::new(
                self.db,
                stmt.id(self.db),
                stmt.get_scope_id(self.db),
                ResolvedStmtKind::Exit,
            )),StmtKind::Return => self.resolved = Some(ResolvedStmt::new(
                self.db,
                stmt.id(self.db),
                stmt.get_scope_id(self.db),
                ResolvedStmtKind::Return,
            )),
            StmtKind::Super => self.resolved = Some(ResolvedStmt::new(
                self.db,
                stmt.id(self.db),
                stmt.get_scope_id(self.db),
                ResolvedStmtKind::Super,
            )),
        }
        ResolveStmtsResult::new(self.db, self.resolved, self.errors)
    }
}

impl<'db> ToProto<'db> for ResolvedStmt<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }
}
