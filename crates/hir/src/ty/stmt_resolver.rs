use crate::check::errors::sem_errors::StmtError;
use crate::def::expressions::expression::ParamAssign;
use crate::def::expressions::statement::{Stmt, StmtKind};
use crate::def::interned::identifier::Ident;
use crate::def::scope::FileScopeId;
use crate::to_proto::{AstId, ToProto};
use crate::ty::expr_resolver::{Env, ResolvedExpr, ResolvedExprKind, resolve_expr};
use crate::ty::ty::TyKind;
use crate::ty::ty_path_expr_resolver::{ResolvedPathResult, resolved_path_expr};
use crate::ty::ty_var_access_resolver::{ResolvedVarResult, resolve_var_access};
use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolve_stmt<'db>(db: &'db dyn BaseDatabase, stmt: Stmt<'db>) -> ResolvedStmt<'db> {
    ResolveStmtCtx::new(db, stmt).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedStmt<'db> {
    pub id: AstId,
    pub scope_id: FileScopeId<'db>,
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
    FuncCall {
        target: ResolvedPathResult<'db>,
        params: Vec<ResolvedParam<'db>>,
    },
    If {
        condition: ResolvedExpr<'db>,
        then: Vec<ResolvedStmt<'db>>,
        else_if: Vec<(ResolvedExpr<'db>, Vec<ResolvedStmt<'db>>)>,
        else_: Vec<ResolvedStmt<'db>>,
    },
    Case {},
    For {
        control_var: ResolvedVarResult<'db>,
        start: ResolvedExpr<'db>,
        end: ResolvedExpr<'db>,
        step: Option<ResolvedExpr<'db>>,
        body: Vec<ResolvedStmt<'db>>,
    },
    Repeat {
        condition: ResolvedExpr<'db>,
        body: Vec<ResolvedStmt<'db>>,
    },
    While {
        condition: ResolvedExpr<'db>,
        body: Vec<ResolvedStmt<'db>>,
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
}

impl<'db> ResolveStmtCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, stmt: Stmt<'db>) -> Self {
        Self { db, stmt }
    }

    pub fn resolve(self) -> ResolvedStmt<'db> {
        match self.stmt.stmt(self.db) {
            StmtKind::Assignment { var, target } => {
                let resolved_var = resolve_var_access(self.db, var);
                let resolved_target = resolve_expr(self.db, *target);

                ResolvedStmt::new(
                    self.db,
                    self.stmt.id(self.db),
                    self.stmt.scope_id(self.db),
                    ResolvedStmtKind::Assignment {
                        var: resolved_var,
                        target: *resolved_target,
                    },
                )
            }
            StmtKind::AssignmentAttempt { var, target } => {
                let resolved_var = resolve_var_access(self.db, var);
                let resolved_target = resolve_expr(self.db, *target);

                ResolvedStmt::new(
                    self.db,
                    self.stmt.id(self.db),
                    self.stmt.scope_id(self.db),
                    ResolvedStmtKind::AssignmentAttempt {
                        var: resolved_var,
                        target: *resolved_target,
                    },
                )
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::If {
                    condition: *resolve_expr(self.db, *condition),
                    then: then
                        .as_ref()
                        .map(|then| then.iter().map(|s| *resolve_stmt(self.db, *s)).collect())
                        .unwrap_or_default(),
                    else_if: else_if
                        .iter()
                        .map(|(cond, stmts)| {
                            (
                                *resolve_expr(self.db, *cond),
                                stmts.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                            )
                        })
                        .collect(),
                    else_: else_
                        .as_ref()
                        .map(|else_| else_.iter().map(|s| *resolve_stmt(self.db, *s)).collect())
                        .unwrap_or_default(),
                },
            ),
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Case {},
            ),
            StmtKind::Invocation { target, params } => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Invocation {},
            ),
            StmtKind::FuncCall { target, params } => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::FuncCall {
                    target: *resolved_path_expr(self.db, *target),
                    params: params.iter().map(|param| match param {
                        ParamAssign::ParamAssignInput { param, value } => {
                            ResolvedParam::Input { param: param.clone(), value: *resolve_expr(self.db, *value) }
                        },
                        ParamAssign::ParamAssignOutput { not, param, variable } => {
                            ResolvedParam::Output { not: *not, param: param.clone(), variable: resolve_var_access(self.db, variable) }
                        } 
                    }).collect(),
                },
            ),
            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body,
            } => {
                let control_var = resolve_var_access(self.db, control_variable);
                let start_expr = resolve_expr(self.db, *start);
                let end_expr = resolve_expr(self.db, *end);
                let step_expr = step.as_ref().map(|s| resolve_expr(self.db, *s));

                ResolvedStmt::new(
                    self.db,
                    self.stmt.id(self.db),
                    self.stmt.scope_id(self.db),
                    ResolvedStmtKind::For {
                        control_var: control_var,
                        start: *start_expr,
                        end: *end_expr,
                        step: step_expr.copied(),
                        body: body.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                    },
                )
            }
            StmtKind::Repeat { body, condition } => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Repeat {
                    condition: *resolve_expr(self.db, *condition),
                    body: body.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                },
            ),
            StmtKind::While { condition, body } => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::While {
                    condition: *resolve_expr(self.db, *condition),
                    body: body.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                },
            ),
            StmtKind::Continue => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Continue,
            ),
            StmtKind::Exit => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Exit,
            ),
            StmtKind::Return => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Return,
            ),
            StmtKind::Super => ResolvedStmt::new(
                self.db,
                self.stmt.id(self.db),
                self.stmt.scope_id(self.db),
                ResolvedStmtKind::Super,
            ),
        }
    }
}

impl<'db> ToProto<'db> for ResolvedStmt<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedParam<'db> {
    Input {
        param: Option<Ident>,
        value: ResolvedExpr<'db>,
    },
    Output {
        not: bool,
        param: Ident,
        variable: ResolvedVarResult<'db>
    },
}