use std::sync::Arc;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::expressions::expression::{Elementary, Expr, ExprKind, PrimaryExpr},
    hir_ty::{
        ty::Ty,
        ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::{resolve_var_access, ResolvedVarResult},
        TyResolved,
    },
};

/// The environment in which the expression is resolved.
/// It can be a concrete type or a boolean context.
/// 
/// Boolean context is used for boolean statements and expressions,
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Env<'db> {
    Ty(Ty<'db>),
    Bool
}

#[salsa::tracked(no_eq)]
pub fn resolve_expr<'db>(
    db: &'db dyn BaseDatabase,
    env: Env<'db>,
    expr: Expr<'db>,
) -> Arc<ResolvedExpr<'db>> {
    Arc::new(ResolveExprCtx::new(db, env, expr).resolve())
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedExpr<'db> {
    pub kind: ResolvedExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedExprKind<'db> {
    // Should have Ty
    PathExpr(Arc<ResolvedPathResult<'db>>),
    VarAccess(Arc<ResolvedVarResult<'db>>),

    // May have Ty (return type)
    FuncCall(Ty<'db>),
    VoidCall(Ty<'db>),

    // Does not have Ty - but elmentary literal that has to be resolved
    Literal(Elementary),

    // Emitted by boolean expressions
    Bool(Expr<'db>),

    // Emitted by comparison expressions
    Compare(Expr<'db>),

    Parenthesized(Arc<ResolvedExpr<'db>>), // Parenthesized expressions

    // Emitted by math expressions
    Math(Expr<'db>),
}

impl<'db> TyResolved<'db> for ResolvedExpr<'db> {
    fn ty(&self) -> Option<Ty<'db>> {
        match &self.kind {
            ResolvedExprKind::PathExpr(path) => path.ty(),
            ResolvedExprKind::VarAccess(var) => var.ty(),
            ResolvedExprKind::FuncCall(ty) => Some(ty.clone()),
            _ => None,
        }
    }
}

pub struct ResolveExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    env: Env<'db>,
    expr: Expr<'db>,
}

impl<'db> ResolveExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, env: Env<'db>, expr: Expr<'db>) -> Self {
        Self { db, env, expr }
    }

    pub fn resolve(&self) -> ResolvedExpr<'db> {
        match self.expr.expr(self.db) {
            ExprKind::PrimaryExpr(primary) => match primary {
                PrimaryExpr::VariableAccess {
                    variable,
                    multibits,
                } => ResolvedExpr {
                    kind: ResolvedExprKind::VarAccess(resolve_var_access(
                        self.db,
                        self.expr.scope_id(self.db),
                        variable,
                    )),
                },
                PrimaryExpr::Literal(lit) => ResolvedExpr {
                    kind: ResolvedExprKind::Literal(*lit),
                },
                PrimaryExpr::ParenthesizedExpr { expr } => ResolvedExpr {
                    kind: ResolvedExprKind::Parenthesized(resolve_expr(
                        self.db,
                        self.env,
                        *expr,
                    )),
                },
                _ => todo!(),
            },
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let left_resolved = resolve_expr(self.db, self.env, *left);
                let right_resolved = resolve_expr(self.db, self.env, *right);
                ResolvedExpr {
                    kind: ResolvedExprKind::Math(self.expr),
                }
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                let left_resolved = resolve_expr(self.db, self.env, *left);
                let right_resolved = resolve_expr(self.db, self.env, *right);
                ResolvedExpr {
                    kind: ResolvedExprKind::Bool(self.expr),
                }
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                let left_resolved = resolve_expr(self.db, self.env, *left);
                let right_resolved = resolve_expr(self.db, self.env, *right);
                ResolvedExpr {
                    kind: ResolvedExprKind::Compare(self.expr),
                }
            }
            _ => todo!(),
        }
    }
}
