use std::fmt::Display;
use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::check::errors::path_error::PathResolveError;
use crate::hir_def::expressions::expression::{PathExprKind, VarAccess};
use crate::hir_def::interned::namespace::{NamespaceAccess, NamespacePath};
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::ScopeKind;
use crate::hir_def::semantic_index::semantic_index;
use crate::hir_def::{
    expressions::expression::PathExpr, interned::identifier::SpanIdent, scope::FileScopeId,
};
use crate::hir_ty::name_res::{pou_names_res, resolve_namespace_access, variables_in_scope};
use crate::hir_ty::ty::{Ty, ty_for_pou, ty_for_variable};
use crate::{AstId, HirNodeInfo, TypeInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
    /// Only search in the local scope (e.g., for variable access)
    Local,
    /// Search in the entire global scope (e.g., for fully qualified names)
    Global,
}

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolved_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
) -> ResolvedPathResult<'db> {
    ResolvePathExprCtx::new(db, expr, SearchMode::Global).resolve_path_expr()
}

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolved_local_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
) -> ResolvedPathResult<'db> {
    ResolvePathExprCtx::new(db, expr, SearchMode::Local).resolve_path_expr()
}

#[salsa::tracked(debug)]
pub struct ResolvedPathResult<'db> {
    pub expr: PathExpr<'db>,

    #[tracked]
    #[returns(ref)]
    #[no_eq]
    pub elements: Vec<ResolvedPathElement<'db>>,
}

impl<'db> ResolvedPathResult<'db> {
    pub fn ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, PathResolveError<'db>> {
        match self.elements(db).last() {
            Some(element) => match &element.kind {
                ResolvedPathElementKind::Ty(ty) => Ok(*ty),
                ResolvedPathElementKind::Error(err) => Err(err.clone()),
            },
            None => Err(PathResolveError::NoItemInScope {
                expr: self.expr(db),
                scope: self.expr(db).scope_id(db),
            }),
        }
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
    Error(PathResolveError<'db>),
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
/// Tries to resolve a [`PathExpr`] to a sequence of [`PathExprWalkStep`] steps.
pub struct ResolvePathExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    expr: PathExpr<'db>,
    elements: Vec<ResolvedPathElement<'db>>,
    fragments: Vec<SpanIdent<'db>>,
    target: Option<Ty<'db>>,
    search_mode: SearchMode,
}

impl<'db> ResolvePathExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, expr: PathExpr<'db>, search_mode: SearchMode) -> Self {
        Self {
            db,
            expr,
            elements: vec![],
            fragments: vec![],
            target: None,
            search_mode,
        }
    }

    pub fn resolve_path_expr(mut self) -> ResolvedPathResult<'db> {
        let current_path = self.expr.flatten_steps(self.db);

        'resolve: for (index, step) in current_path.iter().enumerate() {
            match &self.target {
                // Both target and path are available
                // We need to resolve the target step by step
                Some(sig) => {
                    let steps = &current_path[index..];
                    let mut current = *sig;

                    // We need to track which step we're processing
                    // For the base variable, we use the first step's expression
                    // For subsequent steps, we use each step's expression

                    if let Some(first_step) = current_path.get(0) {
                        // Add the initial target type with the base variable expression
                        self.elements.push(ResolvedPathElement::new(
                            *first_step.get_expr(),
                            ResolvedPathElementKind::Ty(current),
                        ));
                    }

                    for step in steps {
                        match current.linear(self.db, step) {
                            Ok(next_sig) => {
                                current = next_sig;
                                self.elements.push(ResolvedPathElement::new(
                                    *step.get_expr(),
                                    ResolvedPathElementKind::Ty(current),
                                ));
                            }
                            Err(error) => self.elements.push(ResolvedPathElement::new(
                                *step.get_expr(),
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
                // Only add if no elements were processed (i.e., simple variable access without steps)
                if self.elements.is_empty() {
                    self.elements.push(ResolvedPathElement::new(
                        self.expr,
                        ResolvedPathElementKind::Ty(sig),
                    ));
                }
            }
            None => {
                return ResolvedPathResult::new(
                    self.db,
                    self.expr,
                    vec![ResolvedPathElement::new(
                        self.expr,
                        ResolvedPathElementKind::Error(PathResolveError::NoItemInScope {
                            expr: self.expr,
                            scope: self.expr.scope_id(self.db),
                        }),
                    )],
                );
            }
        }
        ResolvedPathResult::new(self.db, self.expr, self.elements)
    }

    fn find_target(&mut self, identifier: &SpanIdent<'db>) {
        match self.search_mode {
            SearchMode::Local => {
                // Try variables in scope
                if self.seatch_variables_in_scope(identifier).is_break() {
                    return;
                }

                // Special case: FUNCTION can access its own name as a variable
                // This is only valid in FUNCTION POUs and if we are at the first fragment
                if self.elements.is_empty() {
                    if self.try_use_self(identifier).is_break() {
                        return;
                    }
                }
            }
            SearchMode::Global => {
                // Try variables in scope
                if self.seatch_variables_in_scope(identifier).is_break() {
                    return;
                }

                // Special case: FUNCTION can access its own name as a variable
                // This is only valid in FUNCTION POUs and if we are at the first fragment
                if self.elements.is_empty() {
                    if self.try_use_self(identifier).is_break() {
                        return;
                    }
                }

                if self.search_local_pous(identifier).is_break() {
                    return;
                }

                if self.search_namespaces(identifier).is_break() {
                    return;
                }
            }
        }

        // Accumulate fragments if not resolved yet
        self.fragments.push(*identifier);
    }

    fn seatch_variables_in_scope(&mut self, identifier: &SpanIdent<'db>) -> ControlFlow<()> {
        if let Some(variable) =
            variables_in_scope(self.db, self.expr.scope_id(self.db)).get(identifier)
        {
            self.target = Some(ty_for_variable(self.db, *variable));
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn try_use_self(&mut self, identifier: &SpanIdent<'db>) -> ControlFlow<()> {
        let sema = semantic_index(self.db, self.expr.scope_id(self.db).file(self.db));
        let scope = sema.get_scope(self.db, self.expr.scope_id(self.db));
        if let ScopeKind::Pou(pou) = scope.kind {
            if pou.name(self.db) == &identifier.ident {
                self.target = Some(ty_for_pou(self.db, pou));
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    }

    // Try POUs in scope
    fn search_local_pous(&mut self, identifier: &SpanIdent<'db>) -> ControlFlow<()> {
        if let Some(pou) = pou_names_res(self.db, identifier, self.expr.scope_id(self.db)) {
            self.target = Some(ty_for_pou(self.db, pou));
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    // Try to resolve the accumulated fragments as a namespace path
    fn search_namespaces(&mut self, identifier: &SpanIdent<'db>) -> ControlFlow<()> {
        let path = NamespacePath::from((self.db, &self.fragments));
        let access = NamespaceAccess::new(self.db, Some(path), identifier);
        if let Some(pou) = resolve_namespace_access(self.db, self.expr.scope_id(self.db), access) {
            self.target = Some(ty_for_pou(self.db, pou));
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
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
                        ident: *simple,
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
                    ident: simple,
                }),
                VarAccess::Deref(_) => result.push(PathExprWalkStep::Deref { expr: self }),
            },
        }

        result
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedPathElement<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        // We do not use the id field of the expression because of the way PathExpr are built from the AST.
        // Instead, get_id method will return the id of *this* specific path element.
        self.expr.get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr.scope_id(db)
    }
}
