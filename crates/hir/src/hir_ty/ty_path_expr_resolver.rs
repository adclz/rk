use auto_lsp::default::db::BaseDatabase;

use crate::check::errors::path_expr::PathExprError;
use crate::check::errors::sem_errors::{AnalysisError};
use crate::hir_def::expressions::expression::{Expr, PathExprKind, VarAccess};
use crate::hir_def::interned::namespace::{NamespaceAccess, NamespacePath};
use crate::hir_def::{
    expressions::expression::PathExpr, interned::identifier::SpanIdent, scope::FileScopeId,
};
use crate::to_proto::{AstId, ToProto};
use crate::hir_ty::TyInfo;
use crate::hir_ty::name_res::{pous_in_scope, resolve_namespace_access, variables_in_scope};
use crate::hir_ty::ty::{Ty, ty_for_pou, ty_for_variable};

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolved_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
) -> ResolvedPathResult<'db> {
    ResolvePathExprCtx::new(db, expr).resolve_path_expr()
}

#[salsa::tracked(debug)]
pub struct ResolvedPathResult<'db> {
    pub expr: PathExpr<'db>,
    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub elements: Vec<ResolvedPathElement<'db>>,
}

impl<'db> TyInfo<'db> for ResolvedPathResult<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AnalysisError<'db>> {
        match self.elements(db).last() {
            Some(element) => match &element.kind {
                ResolvedPathElementKind::Ty(ty) => Ok(*ty),
                ResolvedPathElementKind::Error(err) => {
                    Err(AnalysisError::PathExprError(err.clone()))
                }
            },
            None => Err(AnalysisError::PathExprError(PathExprError::NoItemInScope {
                expr: self.expr(db),
                scope: self.expr(db).scope_id(db),
            })),
        }
    }

    fn place(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }
}

/// Represents a resolved element in a path expression.
/// Contains the expression and its type.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedPathElement<'db> {
    // The expression that was resolved
    pub expr: PathExpr<'db>,
    // The type of the resolved element
    pub kind: ResolvedPathElementKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedPathElementKind<'db> {
    Ty(Ty<'db>),
    Error(PathExprError<'db>),
}

impl<'db> ResolvedPathElement<'db> {
    pub fn new(expr: PathExpr<'db>, kind: ResolvedPathElementKind<'db>) -> Self {
        Self { expr, kind }
    }

    pub fn get_expr(&self) -> &PathExpr<'db> {
        &self.expr
    }
}

/// [`PathExpr`] resolver.
///
/// Tries to resolve a [`PathExpr`] to a sequence of [`Signature`] steps.
pub struct ResolvePathExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
    fragments: Vec<SpanIdent<'db>>,
    target: Option<Ty<'db>>,
}

impl<'db> ResolvePathExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, expr: PathExpr<'db>) -> Self {
        Self {
            db,
            expr,
            fragments: vec![],
            target: None,
        }
    }

    pub fn resolve_path_expr(mut self) -> ResolvedPathResult<'db> {
        let mut elements = Vec::new();
        let current_path = self.expr.flatten_steps(self.db);

        'resolve: for (index, step) in current_path.iter().enumerate() {
            match &self.target {
                // Both target and path are available
                // We need to resolve the target step by step
                Some(sig) => {
                    let steps = &current_path[index..];
                    let mut current = *sig;

                    for step in steps {
                        match current.linear(self.db, step) {
                            Ok(next_sig) => {
                                current = next_sig;
                                elements.push(ResolvedPathElement::new(
                                    *step.get_expr(),
                                    ResolvedPathElementKind::Ty(current),
                                ));
                            }
                            Err(error) => elements.push(ResolvedPathElement::new(
                                self.expr,
                                ResolvedPathElementKind::Error(error),
                            )),
                        }
                    }
                    break 'resolve; // Exit the loop after resolving the path
                }
                // No target yet
                None => {
                    if let PathExprWalkStep::Field { ident, expr } = step {
                        self.find_target(ident);
                    }
                }
            }
        }

        match self.target {
            Some(sig) => {
                elements.push(ResolvedPathElement::new(
                    self.expr,
                    ResolvedPathElementKind::Ty(sig),
                ));
            }
            None => {
                return ResolvedPathResult::new(
                    self.db,
                    self.expr,
                    vec![ResolvedPathElement::new(
                        self.expr,
                        ResolvedPathElementKind::Error(PathExprError::NoItemInScope {
                            expr: self.expr,
                            scope: self.expr.scope_id(self.db),
                        }),
                    )],
                );
            }
        }

        ResolvedPathResult::new(self.db, self.expr, elements)
    }

    fn find_target(&mut self, identifier: &SpanIdent<'db>) {
        // Try variables in scope
        if let Some(variable) =
            variables_in_scope(self.db, self.expr.scope_id(self.db)).get(identifier)
        {
            self.target = Some(ty_for_variable(self.db, *variable));
            return;
        }

        // Try POUs in scope
        if let Some(pou) = pous_in_scope(self.db, self.expr.scope_id(self.db)).get(identifier) {
            self.target = Some(ty_for_pou(self.db, *pou));
            return;
        }

        // Try namespace resolution
        let path = NamespacePath::from((self.db, &self.fragments));
        let access = NamespaceAccess::new(self.db, Some(path), identifier);
        if let Some(pou) = resolve_namespace_access(self.db, self.expr.scope_id(self.db), access) {
            self.target = Some(ty_for_pou(self.db, pou));
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
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr(db).scope_id(db)
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        if let Ok(ty) = self.ty(db) {
            ty.declaration(db)
        } else {
            None
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        if let Ok(ty) = self.ty(db) {
            ty.definition(db)
        } else {
            None
        }
    }
}
