use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::sem_errors::AnalysisError, def::{
        expressions::expression::{Elementary, Expr, ExprKind, PrimaryExpr},
        scope::FileScopeId,
    }, to_proto::{AstId, ToProto}, ty::{
        ty::{Ty, TyDecl, TyKind}, ty_path_expr_resolver::ResolvedPathResult, ty_var_access_resolver::{resolve_var_access, ResolvedVarResult}, TyInfo
    }
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
    pub id: AstId,
    pub scope_id: FileScopeId<'db>,
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
    FuncCall(Ty<'db>),

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

impl<'db> TyInfo<'db> for ResolvedExpr<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match &self.kind(db) {
            ResolvedExprKind::PathExpr(path) => path.ty(db),
            ResolvedExprKind::VarAccess(var) => var.ty(db),
            ResolvedExprKind::FuncCall(ty) => Some(*ty),
            _ => None,
        }
    }

    fn is_err(&self, db: &'db dyn BaseDatabase) -> Option<AnalysisError<'db>> {
        None
    }

    fn place(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
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
                    self.expr.id(self.db),
                    self.expr.scope_id(self.db),
                    ResolvedExprKind::VarAccess(resolve_var_access(self.db, variable)),
                ),
                PrimaryExpr::Literal(lit) => ResolvedExpr::new(
                    self.db,
                    self.expr.id(self.db),
                    self.expr.scope_id(self.db),
                    ResolvedExprKind::Literal(*lit),
                ),
                PrimaryExpr::ParenthesizedExpr { expr } => ResolvedExpr::new(
                    self.db,
                    self.expr.id(self.db),
                    self.expr.scope_id(self.db),
                    ResolvedExprKind::Parenthesized(*resolve_expr(self.db, *expr)),
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
                    self.expr.id(self.db),
                    self.expr.scope_id(self.db),
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
                    self.expr.id(self.db),
                    self.expr.scope_id(self.db),
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
                    self.expr.id(self.db),
                    self.expr.scope_id(self.db),
                    ResolvedExprKind::Compare(*left_resolved, *right_resolved),
                )
            }
            _ => todo!(),
        }
    }
}

impl<'db> ToProto<'db> for ResolvedExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}
