use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::expression::{InitExpr, InitExprKind, PathExpr, PathExprKind, VarAccess},
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
    pub fn flatten(self, db: &'db dyn BaseDatabase) -> Vec<PathExprWalkStep<'db>> {
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
                        result.push(PathExprWalkStep::Deref { expr: self, count: *count })
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
        db: &'db dyn BaseDatabase,
    ) -> Option<(NamespaceAccess<'db>, Ident)> {
        let flatten = self.flatten(db);

        // Collect FIELD.FIELD.FIELD prefix
        let mut frags = Vec::new();

        for step in flatten.iter().rev() {
            match step {
                PathExprWalkStep::Field { ident, .. } => frags.push(*ident),
                _ => break,
            }
        }

        if frags.is_empty() {
            return None;
        }

        let first = frags.remove(0);

        let scope = self.scope_id(db);
        let access = NamespaceAccess::new(
            db,
            match frags.len() {
                0 => None,
                _ => Some(SpanNamespacePath::from((db, &frags, scope))),
            },
            first,
        );

        Some((access, *first)) // returning `first` is optional
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
        db: &'db dyn BaseDatabase,
    ) -> FxIndexMap<InitExpr<'db>, InitExprWalkStep<'db>> {
        let mut map = FxIndexMap::default();
        self.flat(db, &mut map);
        map
    }

    fn flat(
        &self,
        db: &'db dyn BaseDatabase,
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
