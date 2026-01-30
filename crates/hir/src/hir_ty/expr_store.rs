use std::iter::FusedIterator;

use db::WorkspaceDataBase;

use crate::hir_def::{
    expressions::expression::{
        Elementary, Expr, ExprKind, InitExpr, InitExprKind, PathExpr, PathExprKind, PrimaryExpr,
        VarAccess,
    },
    interned::{
        identifier::{Ident, SpanIdent},
        namespace::{NamespaceAccess, SpanNamespacePath},
    },
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
        self.flat(db)
    }

    fn flat(&self, db: &'db dyn WorkspaceDataBase) -> Vec<PathExprWalkStep<'db>> {
        let mut result = Vec::new();

        match self.expr(db) {
            PathExprKind::Field(field_expr) => {
                result.extend(field_expr.path.flat(db));
                match &field_expr.var {
                    VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                        expr: *self,
                        ident: *simple,
                    }),
                    VarAccess::Deref(target, count) => {
                        result.push(PathExprWalkStep::Field {
                            ident: *target,
                            expr: *self,
                        });
                        result.push(PathExprWalkStep::Deref {
                            expr: *self,
                            count: *count,
                        })
                    }
                }
            }
            PathExprKind::Index(index_expr) => {
                result.extend(index_expr.path.flat(db));
                result.push(PathExprWalkStep::Index { expr: *self });
            }
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                    expr: *self,
                    ident: simple,
                }),
                VarAccess::Deref(target, count) => {
                    // deref behaves similar to a field access
                    // but we don't want to repeat the same logic, so we split it into two steps
                    result.push(PathExprWalkStep::Field {
                        expr: *self,
                        ident: target,
                    });
                    result.push(PathExprWalkStep::Deref { expr: *self, count });
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

#[derive(Clone, Debug, Hash, PartialEq, Eq, salsa::Update)]
pub enum InitExprWalkStep<'db> {
    ArrayInit {
        // Initializer for array
        expr: InitExpr<'db>,
        values: Vec<InitExprWalkStep<'db>>,
    },
    FieldInit {
        // Initializer for struct-like types
        expr: InitExpr<'db>,
        values: Vec<InitExprWalkStep<'db>>,
    },
    SizedIndex {
        // Fill an array with a given size
        expr: InitExpr<'db>,
        size: SpanIdent<'db>,
        values: Vec<InitExprWalkStep<'db>>,
    },
    Field {
        // Accessing a field
        expr: InitExpr<'db>,
        name: SpanIdent<'db>,
        value: Box<InitExprWalkStep<'db>>,
    },
    ConstantExpr {
        // A constant expression (evaluated by the body inference)
        expr: InitExpr<'db>,
        value: Expr<'db>,
    },
}

impl InitExprWalkStep<'_> {
    pub fn get_expr(&self) -> &InitExpr<'_> {
        match self {
            InitExprWalkStep::ArrayInit { expr, .. } => expr,
            InitExprWalkStep::FieldInit { expr, .. } => expr,
            InitExprWalkStep::SizedIndex { expr, .. } => expr,
            InitExprWalkStep::Field { expr, .. } => expr,
            InitExprWalkStep::ConstantExpr { expr, .. } => expr,
        }
    }

    fn children(&self) -> &[InitExprWalkStep<'_>] {
        match self {
            InitExprWalkStep::ArrayInit { values, .. } => values,
            InitExprWalkStep::FieldInit { values, .. } => values,
            InitExprWalkStep::SizedIndex { values, .. } => values,
            InitExprWalkStep::Field { value, .. } => std::slice::from_ref(value),
            InitExprWalkStep::ConstantExpr { .. } => &[],
        }
    }
}

pub struct InitExprIterator<'db> {
    stack: Vec<&'db InitExprWalkStep<'db>>,
}

impl<'db> InitExprIterator<'db> {
    pub fn new(root: &'db InitExprWalkStep<'db>) -> Self {
        Self { stack: vec![root] }
    }
}

impl<'db> Iterator for InitExprIterator<'db> {
    type Item = &'db InitExprWalkStep<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;

        // Push children in reverse so traversal is left-to-right
        for child in node.children().iter().rev() {
            self.stack.push(child);
        }

        Some(node)
    }
}

impl<'db> FusedIterator for InitExprIterator<'db> {}

#[salsa::tracked]
impl<'db> InitExpr<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn flatten(self, db: &'db dyn WorkspaceDataBase) -> Vec<InitExprWalkStep<'db>> {
        self.flat(db)
    }

    fn flat(&self, db: &'db dyn WorkspaceDataBase) -> Vec<InitExprWalkStep<'db>> {
        let mut result = vec![];
        // Infering init expressions can be quite long ...
        db.unwind_if_revision_cancelled();
        match self.kind(db) {
            InitExprKind::ArrayInit { values } => {
                result.push(InitExprWalkStep::ArrayInit {
                    expr: *self,
                    values: values.iter().map(|v| v.flat(db)).flatten().collect(),
                });
            }
            InitExprKind::ArrayIndexedElement { size, values } => {
                result.push(InitExprWalkStep::SizedIndex {
                    expr: *self,
                    size: size.clone(),
                    values: values.iter().map(|v| v.flat(db)).flatten().collect(),
                });
            }
            InitExprKind::StructInit { values } => {
                result.push(InitExprWalkStep::FieldInit {
                    expr: *self,
                    values: values.iter().map(|v| v.flat(db)).flatten().collect(),
                });
            }
            InitExprKind::StructElement { name, value } => {
                result.push(InitExprWalkStep::Field {
                    expr: *self,
                    name: name.clone(),
                    value: Box::new(value.flat(db).pop().unwrap()),
                });
            }
            InitExprKind::ConstantExpr(expr) => {
                result.push(InitExprWalkStep::ConstantExpr {
                    expr: *self,
                    value: expr,
                });
            }
        }
        result
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
