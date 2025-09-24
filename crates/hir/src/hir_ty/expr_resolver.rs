use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::expression::{
            Elementary, Expr, ExprKind, ParamAssignKind, PrimaryExpr, RefAdress, RefValue,
        },
        scope::FileScopeId,
    }, hir_ty::{
        fucn_call_resolver::ResolvedParam, ty_path_expr_resolver::{resolved_path_expr, ResolvedPathResult}, ty_var_access_resolver::{
            resolve_var_access, ResolvedVarKind, ResolvedVarOrigin, ResolvedVarResult
        }, TyInfo
    }, AstId, HirNodeInfo
};

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
    BooleanExpression(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    // Emitted by comparison expressions
    Compare(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    RefValue(ResolvedRefValue), // &value
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedRefValue {
    Null,
    Adress,
}

impl<'db> ResolvedExpr<'db> {
    pub fn is_constant(&self, db: &'db dyn BaseDatabase) -> bool {
        match self.kind(db) {
            ResolvedExprKind::Literal(_) => true,
            ResolvedExprKind::Parenthesized(expr) => expr.is_constant(db),
            ResolvedExprKind::Math(left, right) => left.is_constant(db) && right.is_constant(db),
            ResolvedExprKind::BooleanExpression(left, right) => {
                left.is_constant(db) && right.is_constant(db)
            }
            ResolvedExprKind::Compare(left, right) => left.is_constant(db) && right.is_constant(db),
            _ => false,
        }
    }
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
                    ResolvedExprKind::VarAccess(resolve_var_access(self.db, *variable)),
                ),
                PrimaryExpr::Literal(lit) => {
                    ResolvedExpr::new(self.db, self.expr, ResolvedExprKind::Literal(*lit))
                }
                PrimaryExpr::ParenthesizedExpr { expr } => ResolvedExpr::new(
                    self.db,
                    self.expr,
                    ResolvedExprKind::Parenthesized(*resolve_expr(self.db, *expr)),
                ),
                PrimaryExpr::FuncCall(func_call) => {
                    let resolved_func_call = func_call.resolve_func_call(self.db);
                    ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::FuncCall {
                            target: resolved_func_call.target,
                            params: resolved_func_call.params,
                        },
                    )
                }
                PrimaryExpr::RefValue { value } => match value {
                    RefValue::Null => ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::RefValue(ResolvedRefValue::Null),
                    ),
                    RefValue::Address(adress) => match adress {
                        RefAdress::Instance(instance) => {
                            todo!()
                        }
                        RefAdress::Symbolic(symbolic) => {
                            todo!()
                        }
                    },
                },
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
                    ResolvedExprKind::BooleanExpression(*left_resolved, *right_resolved),
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

impl<'db> HirNodeInfo<'db> for ResolvedExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr(db).scope_id(db)
    }
}
