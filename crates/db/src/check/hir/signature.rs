
use auto_lsp::default::db::{file::File, BaseDatabase};

use crate::hir::{
    expressions::expression::PathExpr,
    interned::{
        identifier::SpannedIdent,
        namespace::{NamespaceAccess, NamespacePath},
    },
    scopes::solver::{pous_in_scope, resolve_namespace_access, variables_in_scope},
    ty::{ty_for_pou, ty_for_variable, Ty, TyStep, WalkError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath<'db> {
    pub elements: Vec<(PathExpr<'db>, Ty<'db>)>,
    pub error: Option<WalkError<'db>>,
}

/// [`PathExpr`] resolver.
///
/// Tries to resolve a [`PathExpr`] to a sequence of [`Signature`] steps.
pub struct ResolvePathExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    file: File,
    expr: &'db PathExpr<'db>,
    fragments: Vec<SpannedIdent>,
    signature: Option<Ty<'db>>,
}

impl<'db> ResolvePathExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, file: File, expr: &'db PathExpr<'db>) -> Self {
        Self {
            db,
            file,
            expr,
            fragments: vec![],
            signature: None,
        }
    }

    pub fn resolve_path_expr(mut self) -> ResolvedPath<'db> {
        let mut elements = Vec::new();
        let current_path = self.expr.flatten_steps(self.db);

        'resolve: for (index, step) in current_path.iter().enumerate() {
            match &self.signature {
                // Both signature and path are available
                // We need to resolve the signature step by step
                Some(sig) => {
                    let steps = &current_path[index..];
                    let mut current = *sig;
                    //elements.push(sig);

                    for step in steps {
                        match current.linear(self.db, step) {
                            Ok(next_sig) => {
                                current = next_sig;
                                elements.push((*step.get_expr(), current));
                            }
                            Err(error) => {
                                return ResolvedPath {
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
                    if let TyStep::Field { ident, expr } = step {
                        self.find_signature(ident);
                    }
                }
            }
        }

        match self.signature {
            Some(sig) => {
                elements.push((*self.expr, sig));
            }
            None => {
                return ResolvedPath {
                    elements,
                    error: Some(WalkError::NoItemInScope {
                        expr: *self.expr,
                        scope: self.expr.scope_id(self.db),
                    }),
                };
            }
        }

        ResolvedPath {
            elements,
            error: None,
        }
    }

    fn find_signature(&mut self, identifier: &SpannedIdent) {
        // Try variables in scope
        if let Some(variable) =
            variables_in_scope(self.db, self.file, self.expr.scope_id(self.db)).get(identifier)
        {
            self.signature = Some(ty_for_variable(self.db, *variable));
            return;
        }

        // Try POUs in scope
        if let Some(pou) =
            pous_in_scope(self.db, self.file, self.expr.scope_id(self.db)).get(identifier)
        {
            self.signature = Some(ty_for_pou(self.db, *pou));
            return;
        }

        // Try namespace resolution
        let path = NamespacePath::from((self.db, &self.fragments));
        let access = NamespaceAccess::new(self.db, Some(path), identifier);
        if let Some(pou) =
            resolve_namespace_access(self.db, self.file, self.expr.scope_id(self.db), access)
        {
            self.signature = Some(ty_for_pou(self.db, pou));
            return;
        }

        // Accumulate fragments if not resolved yet
        self.fragments.push(identifier.clone());
    }
}
