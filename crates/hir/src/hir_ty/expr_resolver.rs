use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::expression::{Elementary, Expr, ExprKind, ParamAssign, PrimaryExpr},
        scope::FileScopeId,
    },
    to_proto::{AstId, ToProto},
    hir_ty::{
        stmt_resolver::ResolvedParam,
        ty::Ty,
        ty_path_expr_resolver::{ResolvedPathResult, resolved_path_expr},
        ty_var_access_resolver::{ResolvedVarResult, resolve_var_access},
    },
};
  
/// The environment in which the expression is resolved.
/// It can be a concrete type or a boolean context.
///
/// Boolean context is used for boolean statements and expressions,
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Env<'db> {
    Ty(Ty<'db>),
    Bool,
}

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolve_expr<'db>(db: &'db dyn BaseDatabase, expr: Expr<'db>) -> ResolvedExpr<'db> {
    ResolveExprCtx::new(db, expr).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedExpr<'db> {
    pub expr: Expr<'db>,
    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub kind: ResolvedExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedExprKind<'db> {
    // Should have Ty
    PathExpr(ResolvedPathResult<'db>),
    VarAccess(ResolvedVarResult<'db>),

    // May have Ty (return type)
    FuncCall {
        target: ResolvedPathResult<'db>,
        params: Vec<ResolvedParam<'db>>,
    },

    // Does not have Ty - but elementary literal that has to be resolved
    Literal(Elementary),

    Parenthesized(ResolvedExpr<'db>), // Parenthesized expressions

    // Emitted by math expressions
    Math(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    // Emitted by boolean expressions
    Bool(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    // Emitted by comparison expressions
    Compare(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right
}

pub struct ResolveExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    expr: Expr<'db>,
}

impl<'db> ResolveExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, expr: Expr<'db>) -> Self {
        Self { db, expr }
    }

    pub fn resolve(&self) -> ResolvedExpr<'db> {
        match self.expr.expr(self.db) {
            ExprKind::PrimaryExpr(primary) => match primary {
                PrimaryExpr::VariableAccess {
                    variable,
                    multibits,
                } => ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::VarAccess(resolve_var_access(self.db, variable)),
                ),
                PrimaryExpr::Literal(lit) => ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::Literal(*lit),
                ),
                PrimaryExpr::ParenthesizedExpr { expr } => ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::Parenthesized(*resolve_expr(self.db, *expr)),
                ),
                PrimaryExpr::FuncCall { path, params } => ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::FuncCall {
                        target: *resolved_path_expr(self.db, *path),
                        params: params
                            .iter()
                            .map(|param| match param {
                                ParamAssign::ParamAssignInput { param, value } => {
                                    ResolvedParam::Input {
                                        param: *param,
                                        value: *resolve_expr(self.db, *value),
                                    }
                                }
                                ParamAssign::ParamAssignOutput {
                                    not,
                                    param,
                                    variable,
                                } => ResolvedParam::Output {
                                    not: *not,
                                    param: *param,
                                    variable: resolve_var_access(self.db, variable),
                                },
                            })
                            .collect(),
                    },
                ),
                _ => todo!(),
            },
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let left_resolved = resolve_expr(self.db, *left);
                let right_resolved = resolve_expr(self.db, *right);
                ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::Math(*left_resolved, *right_resolved),
                )
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                let left_resolved = resolve_expr(self.db, *left);
                let right_resolved = resolve_expr(self.db, *right);
                ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::Bool(*left_resolved, *right_resolved),
                )
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                let left_resolved = resolve_expr(self.db, *left);
                let right_resolved = resolve_expr(self.db, *right);
                ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::Compare(*left_resolved, *right_resolved),
                )
            }
            _ => todo!(),
        }
    }
}

impl<'db> ToProto<'db> for ResolvedExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr(db).scope_id(db)
    }
}
