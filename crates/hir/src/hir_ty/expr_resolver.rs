use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PathExpr, PrimaryExpr, RefValue},
            spec::EnumVariant,
        },
        interned::identifier::SpanIdent,
        scope::FileScopeId,
        semantic_index::semantic_index,
    },
    hir_ty::{
        func_call_resolver::ResolvedFuncCall,
        invocation_resolver::ResolvedInvocationResult,
        ty::TyKind,
        ty_var_access_resolver::{ResolvedAccess, resolve_path_expr, resolve_var_access},
        walk::ResolvedPath,
    },
};

#[salsa::tracked]
pub fn resolve_expr<'db>(db: &'db dyn BaseDatabase, expr: Expr<'db>) -> ResolvedExpr<'db> {
    ResolveExprCtx::new(db, expr).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedExpr<'db> {
    pub expr: Expr<'db>,

    pub kind: ResolvedExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedExprKind<'db> {
    // Should have Ty
    VarAccess(ResolvedAccess<'db>),

    Invocation(ResolvedInvocationResult<'db>),

    // May have Ty (return type)
    FuncCall(ResolvedFuncCall<'db>),

    // Does not have Ty - but elementary literal that has to be resolved
    Literal(Elementary),

    Parenthesized(ResolvedExpr<'db>), // Parenthesized expressions

    // Emitted by math expressions
    Math(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    // Emitted by boolean expressions
    BooleanExpression(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    // Emitted by comparison expressions
    Compare(ResolvedExpr<'db>, ResolvedExpr<'db>), // left, right

    // Enum value (#variant)
    EnumValue {
        name: ResolvedAccess<'db>,
        v_text: SpanIdent<'db>,
        variant: Option<EnumVariant<'db>>,
    },

    RefValue(ResolvedRefValue<'db>), // &value
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedRefValue<'db> {
    Null,
    Adress(ResolvedAccess<'db>),
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
                    ResolvedExprKind::Parenthesized(resolve_expr(self.db, *expr)),
                ),
                PrimaryExpr::Invocation(invocation) => {
                    let scope = semantic_index(self.db, self.expr.scope_id(self.db).file(self.db))
                        .get_scope(self.db, self.expr.scope_id(self.db));
                    let resolved_invocation = invocation.resolve_invocation(self.db, scope);

                    ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::Invocation(resolved_invocation),
                    )
                }
                PrimaryExpr::FuncCall(func_call) => {
                    let resolved_func_call = func_call.resolve_func_call(self.db);
                    ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::FuncCall(resolved_func_call),
                    )
                }
                PrimaryExpr::EnumValue { name, variant } => {
                    // Find the target enum type
                    let resolved_path = resolve_path_expr(self.db, *name);

                    // Find the variant in the enum type
                    let resolved_variant = if let Ok(Some(TyKind::Enum(enum_))) = resolved_path
                        .resolved(self.db)
                        .map(|p| p.to_ty(self.db).map(|t| t.kind(self.db)))
                    {
                        enum_
                            .variants
                            .iter()
                            .find(|v| v.name == variant.ident)
                            .cloned()
                    } else {
                        None
                    };

                    ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::EnumValue {
                            name: resolved_path,
                            v_text: variant.clone(),
                            variant: resolved_variant,
                        },
                    )
                }
                PrimaryExpr::RefValue { value } => match value {
                    RefValue::Null => ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::RefValue(ResolvedRefValue::Null),
                    ),
                    RefValue::Address(adress) => ResolvedExpr::new(
                        self.db,
                        self.expr,
                        ResolvedExprKind::RefValue(ResolvedRefValue::Adress(resolve_path_expr(
                            self.db,
                            adress.kind,
                        ))),
                    ),
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
                    ResolvedExprKind::Math(left_resolved, right_resolved),
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
                    ResolvedExprKind::BooleanExpression(left_resolved, right_resolved),
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
                    ResolvedExprKind::Compare(left_resolved, right_resolved),
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
