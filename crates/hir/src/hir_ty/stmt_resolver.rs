use crate::hir_def::expressions::expression::{Expr, ParamAssign, ParamAssignKind};
use crate::hir_def::expressions::invocation::InvocationKind;
use crate::hir_def::expressions::statement::{Stmt, StmtKind};
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::scope::FileScopeId;
use crate::hir_def::semantic_index::semantic_index;
use crate::hir_ty::expr_resolver::{ResolvedExpr, resolve_expr};
use crate::hir_ty::func_call_resolver::{ResolvedFuncCall, ResolvedParam};
use crate::hir_ty::invocation_resolver::{ResolvedInvocation, ResolvedInvocationResult, ResolvedMethodKind};
use crate::hir_ty::ty::{Ty, TyDecl};
use crate::hir_ty::ty_path_expr_resolver::{ResolvedPathResult, resolved_path_expr};
use crate::hir_ty::ty_var_access_resolver::{
    ResolvedVarKind, ResolvedVarOrigin, ResolvedVarResult, resolve_var_access,
};
use crate::{AstId, HirNodeInfo};
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolve_stmt<'db>(db: &'db dyn BaseDatabase, stmt: Stmt<'db>) -> ResolvedStmt<'db> {
    ResolveStmtCtx::new(db, stmt).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedStmt<'db> {
    pub stmt: Stmt<'db>,

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
    Invocation(ResolvedInvocationResult<'db>),
    FuncCall(ResolvedFuncCall<'db>),
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
                let resolved_var = resolve_var_access(self.db, *var);
                let resolved_target = resolve_expr(self.db, *target);

                ResolvedStmt::new(
                    self.db,
                    self.stmt,
                    ResolvedStmtKind::Assignment {
                        var: resolved_var,
                        target: *resolved_target,
                    },
                )
            }
            StmtKind::AssignmentAttempt { var, target } => {
                let resolved_var = resolve_var_access(self.db, *var);
                let resolved_target = resolve_expr(self.db, *target);

                ResolvedStmt::new(
                    self.db,
                    self.stmt,
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
                self.stmt,
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
            } => ResolvedStmt::new(self.db, self.stmt, ResolvedStmtKind::Case {}),
            StmtKind::Invocation(invocation) => {
                let scope = semantic_index(self.db, self.stmt.scope_id(self.db).file(self.db))
                    .get_scope(self.db, self.stmt.scope_id(self.db));
                let resolved_invocation = invocation.resolve_invocation(self.db, scope);

                ResolvedStmt::new(
                    self.db,
                    self.stmt,
                    ResolvedStmtKind::Invocation(resolved_invocation),
                )
            }
            StmtKind::FuncCall(func_call) => {
                let resolved_func_call = func_call.resolve_func_call(self.db);
                ResolvedStmt::new(
                    self.db,
                    self.stmt,
                    ResolvedStmtKind::FuncCall(resolved_func_call),
                )
            }
            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body,
            } => {
                let control_var = resolve_var_access(self.db, *control_variable);
                let start_expr = resolve_expr(self.db, *start);
                let end_expr = resolve_expr(self.db, *end);
                let step_expr = step.as_ref().map(|s| resolve_expr(self.db, *s));

                ResolvedStmt::new(
                    self.db,
                    self.stmt,
                    ResolvedStmtKind::For {
                        control_var,
                        start: *start_expr,
                        end: *end_expr,
                        step: step_expr.copied(),
                        body: body.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                    },
                )
            }
            StmtKind::Repeat { body, condition } => ResolvedStmt::new(
                self.db,
                self.stmt,
                ResolvedStmtKind::Repeat {
                    condition: *resolve_expr(self.db, *condition),
                    body: body.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                },
            ),
            StmtKind::While { condition, body } => ResolvedStmt::new(
                self.db,
                self.stmt,
                ResolvedStmtKind::While {
                    condition: *resolve_expr(self.db, *condition),
                    body: body.iter().map(|s| *resolve_stmt(self.db, *s)).collect(),
                },
            ),
            StmtKind::Continue => ResolvedStmt::new(self.db, self.stmt, ResolvedStmtKind::Continue),
            StmtKind::Exit => ResolvedStmt::new(self.db, self.stmt, ResolvedStmtKind::Exit),
            StmtKind::Return => ResolvedStmt::new(self.db, self.stmt, ResolvedStmtKind::Return),
        }
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedStmt<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.stmt(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.stmt(db).scope_id(db)
    }
}
