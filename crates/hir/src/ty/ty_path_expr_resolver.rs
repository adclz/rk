use std::sync::Arc;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;

use crate::def::expressions::expression::{PathExprKind, VarAccess};
use crate::def::interned::namespace::{NamespaceAccess, NamespacePath};
use crate::to_proto::{AstId, ToProto};
use crate::ty::TyResolved;
use crate::ty::name_res::{pous_in_scope, resolve_namespace_access, variables_in_scope};
use crate::ty::ty::{Ty, ty_for_pou, ty_for_variable};
use crate::{
    def::{
        expressions::expression::PathExpr, interned::identifier::SpanIdent, scope::FileScopeId,
    },
    ty::ty::TyOrigin,
};

#[salsa::tracked(no_eq)]
pub fn resolved_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
) -> Arc<ResolvedPathResult<'db>> {
    Arc::new(ResolvePathExprCtx::new(db, expr).resolve_path_expr())
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedPathResult<'db> {
    pub id: AstId,
    pub scope_id: FileScopeId<'db>,
    pub elements: Vec<ResolvedPathElement<'db>>,
    pub error: Option<PathExprWalkError<'db>>,
}

impl<'db> TyResolved<'db> for ResolvedPathResult<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        self.elements.last().map(|e| e.get_ty()).copied()
    }
}

/// Represents a resolved element in a path expression.
/// Contains the expression and its type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedPathElement<'db> {
    // The expression that was resolved
    pub expr: PathExpr<'db>,
    // The type of the resolved element
    pub ty: Ty<'db>,
}

impl<'db> ResolvedPathElement<'db> {
    pub fn new(expr: PathExpr<'db>, ty: Ty<'db>) -> Self {
        Self { expr, ty }
    }

    pub fn get_expr(&self) -> &PathExpr<'db> {
        &self.expr
    }

    pub fn get_ty(&self) -> &Ty<'db> {
        &self.ty
    }
}

/// [`PathExpr`] resolver.
///
/// Tries to resolve a [`PathExpr`] to a sequence of [`Signature`] steps.
pub struct ResolvePathExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
    fragments: Vec<SpanIdent<'db>>,
    signature: Option<Ty<'db>>,
}

impl<'db> ResolvePathExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, expr: PathExpr<'db>) -> Self {
        Self {
            db,
            expr,
            fragments: vec![],
            signature: None,
        }
    }

    pub fn resolve_path_expr(mut self) -> ResolvedPathResult<'db> {
        let mut elements = Vec::new();
        let current_path = self.expr.flatten_steps(self.db);

        'resolve: for (index, step) in current_path.iter().enumerate() {
            match &self.signature {
                // Both signature and path are available
                // We need to resolve the signature step by step
                Some(sig) => {
                    let steps = &current_path[index..];
                    let mut current = *sig;

                    for step in steps {
                        match current.linear(self.db, step) {
                            Ok(next_sig) => {
                                current = next_sig;
                                elements.push(ResolvedPathElement::new(*step.get_expr(), current));
                            }
                            Err(error) => {
                                return ResolvedPathResult {
                                    id: self.expr.id(self.db),
                                    scope_id: self.expr.scope_id(self.db),
                                    elements,
                                    error: Some(error),
                                };
                            }
                        }
                    }
                    break 'resolve; // Exit the loop after resolving the path
                }
                // No signature yet
                None => {
                    if let PathExprWalkStep::Field { ident, expr } = step {
                        self.find_signature(ident);
                    }
                }
            }
        }

        match self.signature {
            Some(sig) => {
                elements.push(ResolvedPathElement::new(self.expr, sig));
            }
            None => {
                return ResolvedPathResult {
                    id: self.expr.id(self.db),
                    scope_id: self.expr.scope_id(self.db),
                    elements,
                    error: Some(PathExprWalkError::NoItemInScope {
                        expr: self.expr,
                        scope: self.expr.scope_id(self.db),
                    }),
                };
            }
        }

        ResolvedPathResult {
            id: self.expr.id(self.db),
            scope_id: self.expr.scope_id(self.db),
            elements,
            error: None,
        }
    }

    fn find_signature(&mut self, identifier: &SpanIdent<'db>) {
        // Try variables in scope
        if let Some(variable) =
            variables_in_scope(self.db, self.expr.scope_id(self.db)).get(identifier)
        {
            self.signature = Some(ty_for_variable(self.db, *variable));
            return;
        }

        // Try POUs in scope
        if let Some(pou) =
            pous_in_scope(self.db, self.expr.scope_id(self.db)).get(identifier)
        {
            self.signature = Some(ty_for_pou(self.db, *pou));
            return;
        }

        // Try namespace resolution
        let path = NamespacePath::from((self.db, &self.fragments));
        let access = NamespaceAccess::new(self.db, Some(path), identifier);
        if let Some(pou) =
            resolve_namespace_access(self.db, self.expr.scope_id(self.db), access)
        {
            self.signature = Some(ty_for_pou(self.db, pou));
            return;
        }

        // Accumulate fragments if not resolved yet
        self.fragments.push(identifier.clone());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
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
    }, // For pointers or references
}

impl PathExprWalkStep<'_> {
    pub fn get_expr(&self) -> &PathExpr<'_> {
        match self {
            PathExprWalkStep::Field { expr, .. } => expr,
            PathExprWalkStep::Index { expr } => expr,
            PathExprWalkStep::Deref { expr } => expr,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum PathExprWalkError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: FileScopeId<'db>,
    },
    FieldNotFound {
        expr: PathExpr<'db>,
        origin: TyOrigin<'db>,
    },
    NotAnArray {
        expr: PathExpr<'db>,
        origin: TyOrigin<'db>,
    },
    NotAReference {
        expr: PathExpr<'db>,
        origin: TyOrigin<'db>,
    },
}

#[salsa::tracked]
impl<'db> PathExpr<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn flatten_steps(self, db: &'db dyn BaseDatabase) -> Vec<PathExprWalkStep<'db>> {
        let mut result = Vec::new();

        match self.expr(db) {
            PathExprKind::Field(field_expr) => {
                result.extend(field_expr.path.flatten_steps(db).iter().cloned());
                match &field_expr.var {
                    VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                        expr: self,
                        ident: simple.clone(),
                    }),
                    VarAccess::Deref(_) => result.push(PathExprWalkStep::Deref { expr: self }),
                }
            }
            PathExprKind::Index(index_expr) => {
                result.extend(index_expr.path.flatten_steps(db).iter().cloned());
                result.push(PathExprWalkStep::Index { expr: self });
            }
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                    expr: self,
                    ident: simple.clone(),
                }),
                VarAccess::Deref(_) => result.push(PathExprWalkStep::Deref { expr: self }),
            },
        }

        result
    }
}

impl<'db> ToProto<'db> for ResolvedPathResult<'db> {
    fn get_id(&'db self, _db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id
    }
} 
