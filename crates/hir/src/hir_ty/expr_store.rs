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

impl<'db> PathExprWalkStep<'db> {
    pub fn get_expr(&self, db: &'db dyn WorkspaceDataBase) -> PathExpr<'db> {
        *match self {
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
                }
            }
            PathExprKind::Index(index_expr) => {
                result.extend(index_expr.path.flat(db));
                result.push(PathExprWalkStep::Index { expr: *self });
            }
            PathExprKind::Deref(deref_expr) => {
                result.extend(deref_expr.path.flat(db));
                result.push(PathExprWalkStep::Deref {
                    expr: *self,
                    count: deref_expr.count,
                });
            }
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                    expr: *self,
                    ident: simple,
                }),
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
                    values: values.iter().flat_map(|v| v.flat(db)).collect(),
                });
            }
            InitExprKind::ArrayIndexedElement { size, values } => {
                result.push(InitExprWalkStep::SizedIndex {
                    expr: *self,
                    size,
                    values: values.iter().flat_map(|v| v.flat(db)).collect(),
                });
            }
            InitExprKind::StructInit { values } => {
                result.push(InitExprWalkStep::FieldInit {
                    expr: *self,
                    values: values.iter().flat_map(|v| v.flat(db)).collect(),
                });
            }
            InitExprKind::StructElement { name, value } => {
                result.push(InitExprWalkStep::Field {
                    expr: *self,
                    name,
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
    /// Evaluate this expression as a compile-time integer.
    ///
    /// The single const-integer evaluator: array and subrange bounds, enum
    /// values and STRING capacities are all folded through here, so what
    /// validation accepts and what lowering reads can never diverge. Accepts an
    /// integer literal of any typed or untyped form (`10`, `INT#10`, `16#F`,
    /// `2#1010`, `-5`).
    ///
    /// Named constants are not folded yet; a non-literal simply yields `None`.
    pub fn as_const_int(self, db: &'db dyn WorkspaceDataBase) -> Option<i64> {
        let ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) = self.expr(db) else {
            return None;
        };
        match lit {
            Elementary::InferInteger(v)
            | Elementary::SInt(v)
            | Elementary::Int(v)
            | Elementary::DInt(v)
            | Elementary::LInt(v)
            | Elementary::USInt(v)
            | Elementary::UInt(v)
            | Elementary::UDInt(v)
            | Elementary::ULInt(v)
            | Elementary::Byte(v)
            | Elementary::Word(v)
            | Elementary::DWord(v)
            | Elementary::LWord(v) => v.as_i64(db).ok(),
            _ => None,
        }
    }

    /// Evaluate as a compile-time SIZE — a non-negative constant integer.
    ///
    /// Same evaluator as [`Self::as_const_int`], restricted to values a length
    /// or capacity can take.
    pub fn as_range(self, db: &'db dyn WorkspaceDataBase) -> Option<u64> {
        self.as_const_int(db).filter(|v| *v >= 0).map(|v| v as u64)
    }

    /// [`Self::as_const_int`] extended over a unary sign and parentheses —
    /// the shapes a written-out constant takes (`-1`, `+(2)`), which the bare
    /// literal evaluator does not see. Named constants still yield `None`.
    pub fn as_const_int_folded(self, db: &'db dyn WorkspaceDataBase) -> Option<i64> {
        match self.expr(db) {
            ExprKind::UnaryOperator { expr, operator } => match operator {
                crate::hir_def::expressions::expression::UnaryOperatorKind::Minus => {
                    expr.as_const_int_folded(db).and_then(i64::checked_neg)
                }
                crate::hir_def::expressions::expression::UnaryOperatorKind::Plus => {
                    expr.as_const_int_folded(db)
                }
                _ => None,
            },
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
                expr.as_const_int_folded(db)
            }
            _ => self.as_const_int(db),
        }
    }
}
