use auto_lsp::default::db::BaseDatabase;

use crate::{hir_def::{
    expressions::expression::{Expr, InitExpr, InitExprKind, PathExpr, PathExprKind, VarAccess},
    interned::identifier::SpanIdent,
}};

// Some nodes in the HIR are parsed with right precedence (such as initializer or path expressions).
// Therefore it is necessary to flatten them into a more convenient structure.
// Since this operation only depends on the structural definition of the expression, we can easily cache the result.

pub trait Flatten<'db>: Copy {
    type Output;

    fn flatten(self, db: &'db dyn BaseDatabase) -> Self::Output;
}

/// Flattened representation of an initializer expression before type resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct UnResolvedInitExpr<'db> {
    pub expr: InitExpr<'db>,
    pub kind: UnResolvedInitExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnResolvedInitExprKind<'db> {
    ArrayInit {
        values: Vec<UnResolvedInitExpr<'db>>,
    },
    ArrayIndexedElement {
        size: SpanIdent<'db>,
        values: Vec<UnResolvedInitExpr<'db>>,
    },
    StructInit {
        values: Vec<UnResolvedInitExpr<'db>>,
    },
    StructElement {
        name: SpanIdent<'db>,
        value: Box<UnResolvedInitExpr<'db>>,
    },
    ConstantExpr(Expr<'db>),
}

#[salsa::tracked]
impl<'db> Flatten<'db> for InitExpr<'db> {
    type Output = UnResolvedInitExpr<'db>;

    #[salsa::tracked]  
    fn flatten(self, db: &'db dyn BaseDatabase) -> UnResolvedInitExpr<'db> {
        let kind = match self.kind(db) {
            InitExprKind::ArrayInit { values } => {
                let resolved = values.iter().map(|v| v.flatten(db)).collect();
                UnResolvedInitExprKind::ArrayInit { values: resolved }
            }
            InitExprKind::ArrayIndexedElement { size, values } => {
                let resolved = values.iter().map(|v| v.flatten(db)).collect();
                UnResolvedInitExprKind::ArrayIndexedElement {
                    size,
                    values: resolved,
                }
            }
            InitExprKind::StructInit { values } => {
                let resolved = values.iter().map(|v| v.flatten(db)).collect();
                UnResolvedInitExprKind::StructInit { values: resolved }
            }
            InitExprKind::StructElement { name, value } => {
                let resolved = Box::new(value.flatten(db));
                UnResolvedInitExprKind::StructElement {
                    name,
                    value: resolved,
                }
            }
            InitExprKind::ConstantExpr(expr) => UnResolvedInitExprKind::ConstantExpr(expr),
        };
        UnResolvedInitExpr { expr: self, kind }
    }
}

/// Flattened representation of a path expression.
/// Each step in the path is represented as a [`PathExprWalkStep`].
/// 
/// This is later used by type resolution to walk trough a path expression step by step.
#[derive(Debug, Copy, Clone, PartialEq, Eq, salsa::Update)]
pub enum PathExprWalkStep<'db> {
    Field {
        ident: SpanIdent<'db>,
        expr: PathExpr<'db>,
    }, // By name
    Index {
        expr: PathExpr<'db>,
    }, // By index
    Deref {
        target: SpanIdent<'db>,
        expr: PathExpr<'db>,
    }, // For pointers
}

impl PathExprWalkStep<'_> {
    pub fn get_expr(&self) -> &PathExpr<'_> {
        match self {
            PathExprWalkStep::Field { expr, .. } => expr,
            PathExprWalkStep::Index { expr } => expr,
            PathExprWalkStep::Deref { expr, .. } => expr,
        }
    }
}

#[salsa::tracked]
impl<'db> Flatten<'db> for PathExpr<'db> {
    type Output = &'db Vec<PathExprWalkStep<'db>>;

    #[salsa::tracked(returns(ref))]
    fn flatten(self, db: &'db dyn BaseDatabase) -> Vec<PathExprWalkStep<'db>> {
        let mut result = Vec::new();

        match self.expr(db) {
            PathExprKind::Field(field_expr) => {
                result.extend(field_expr.path.flatten(db).iter().cloned());
                match &field_expr.var {
                    VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                        expr: self,
                        ident: *simple,
                    }),
                    VarAccess::Deref(target) => result.push(PathExprWalkStep::Deref {
                        expr: self,
                        target: *target,
                    }),
                }
            }
            PathExprKind::Index(index_expr) => {
                result.extend(index_expr.path.flatten(db).iter().cloned());
                result.push(PathExprWalkStep::Index { expr: self });
            }
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                    expr: self,
                    ident: simple,
                }),
                VarAccess::Deref(target) => {
                    result.push(PathExprWalkStep::Deref { expr: self, target })
                }
            },
        }
        result
    }
}
