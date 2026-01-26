use db::WorkspaceDataBase;

use crate::{
    hir_def::{
        expressions::expression::{
            Elementary, Expr, ExprKind, InitExpr, InitExprKind, PathExpr, PathExprKind,
            PrimaryExpr, VarAccess,
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespaceAccess, SpanNamespacePath},
        },
    },
    hir_ty::def_map::FxIndexMap,
};

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
        expr: PathExpr<'db>,
        count: u16,
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
impl<'db> PathExpr<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn flatten(self, db: &'db dyn WorkspaceDataBase) -> Vec<PathExprWalkStep<'db>> {
        let mut result = Vec::new();

        match self.expr(db) {
            PathExprKind::Field(field_expr) => {
                result.extend(field_expr.path.flatten(db).iter().cloned());
                match &field_expr.var {
                    VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                        expr: self,
                        ident: *simple,
                    }),
                    VarAccess::Deref(target, count) => {
                        result.push(PathExprWalkStep::Field {
                            ident: *target,
                            expr: self,
                        });
                        result.push(PathExprWalkStep::Deref {
                            expr: self,
                            count: *count,
                        })
                    }
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
                VarAccess::Deref(target, count) => {
                    // deref behaves similar to a field access
                    // but we don't want to repeat the same logic, so we split it into two steps
                    result.push(PathExprWalkStep::Field {
                        expr: self,
                        ident: target,
                    });
                    result.push(PathExprWalkStep::Deref { expr: self, count });
                }
            },
        }
        result
    }

    #[salsa::tracked(returns(ref))]
    pub fn to_namespace_access(
        self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<(NamespaceAccess<'db>, Ident)> {
        let flatten = self.flatten(db);

        // Extract only FIELD accesses (ignore INDEX and DEREF)
        let mut field_idents: Vec<SpanIdent<'db>> = flatten
            .iter()
            .filter_map(|step| match step {
                PathExprWalkStep::Field { ident, .. } => Some(*ident),
                _ => None,
            })
            .collect();

        if field_idents.is_empty() {
            return None;
        }

        // NamespaceAccess expects: target (last ident) + prefix (all before)
        // flatten gives us [a, b, c, d] for a.b.c.d
        // We want: target=d, prefix=[a, b, c]
        let target = field_idents.pop().unwrap();

        let scope = self.scope_id(db);
        let access = NamespaceAccess::new(
            db,
            match field_idents.is_empty() {
                true => None,
                false => Some(SpanNamespacePath::from((db, &field_idents, scope))),
            },
            target,
        );

        Some((access, *target))
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, salsa::Update)]
pub enum InitExprWalkStep<'db> {
    Index,
    Access,
    Field(SpanIdent<'db>),
    NoOp,
}

#[salsa::tracked]
impl<'db> InitExpr<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn flatten(
        self,
        db: &'db dyn WorkspaceDataBase,
    ) -> FxIndexMap<InitExpr<'db>, InitExprWalkStep<'db>> {
        let mut map = FxIndexMap::default();
        self.flat(db, &mut map);
        map
    }

    fn flat(
        &self,
        db: &'db dyn WorkspaceDataBase,
        map: &mut FxIndexMap<InitExpr<'db>, InitExprWalkStep<'db>>,
    ) {
        // Infering init expressions can be quite long ...
        db.unwind_if_revision_cancelled();
        match self.kind(db) {
            InitExprKind::ArrayInit { values } => {
                map.insert(*self, InitExprWalkStep::Index);
                for v in values {
                    v.flat(db, map);
                }
            }
            InitExprKind::ArrayIndexedElement { size, values } => {
                for v in values {
                    v.flat(db, map);
                }
            }
            InitExprKind::StructInit { values } => {
                map.insert(*self, InitExprWalkStep::Access);
                for v in values {
                    v.flat(db, map);
                }
            }
            InitExprKind::StructElement { name, value } => {
                map.insert(*self, InitExprWalkStep::Field(name));
                value.flat(db, map);
            }
            InitExprKind::ConstantExpr(expr) => {
                map.insert(*self, InitExprWalkStep::NoOp);
            }
        }
    }
}

impl<'db> Expr<'db> {
    pub fn as_range(self, db: &'db dyn WorkspaceDataBase) -> Option<u64> {
        match self.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(v))) => {
                v.as_u64(db).ok()
            }
            _ => None,
        }
    }
}
