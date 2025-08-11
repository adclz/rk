use std::sync::Arc;

use auto_lsp::default::db::{file::File, BaseDatabase};
use compact_str::CompactString;

use crate::{
    check::{errors::semantic_errors::unexpected_index_expression, hir::signature},
    hir::{
        expressions::{
            expression::{ParamAssign, PathExpr, PathExprKind, VarAccess},
            spec::Spec,
        },
        interned::{
            identifier::{Ident, SpannedIdent},
            namespace::{NamespaceAccess, NamespacePath},
        },
        pous::variable::VariableKind,
        scopes::solver::{pous_in_scope, resolve_namespace_access, variables_in_scope},
        semantic_index::SemanticIndex,
        signature::{
            signature_for_pou, signature_for_variable, LinearError, Signature, SignatureStep,
            WalkSignature,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedPathElement<'db> {
    Signature(Arc<Signature<'db>>),
    Error {
        step: Option<SignatureStep<'db>>,
        kind: LinearError<'db>,
    },
}

pub struct ResolvedPath<'db> {
    pub elements: Vec<ResolvedPathElement<'db>>,
}

/// [`PathExpr`] resolver.
///
/// Tries to resolve a [`PathExpr`] to a sequence of [`Signature`] steps.
pub struct ResolvePathExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    file: File,
    expr: &'db PathExpr<'db>,
    fragments: Vec<SpannedIdent>,
    signature: Option<Arc<Signature<'db>>>,
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
        let mut current_path = self.expr.flatten_steps(self.db);

        'resolve: for (index, step) in current_path.iter().enumerate() {
            match &self.signature {
                // Both signature and path are available
                // We need to resolve the signature step by step
                Some(sig) => {
                    let steps = &current_path[index..];
                    let mut current = Arc::new(sig.as_ref().clone());
                    elements.push(ResolvedPathElement::Signature(current.clone()));

                    for step in steps {
                        match current.linear(step.clone()) {
                            Ok(next_sig) => {
                                current = next_sig.clone();
                                elements.push(ResolvedPathElement::Signature(current.clone()));
                            }
                            Err(e) => {
                                elements.push(ResolvedPathElement::Error {
                                    step: Some(step.clone()),
                                    kind: e,
                                });
                            }
                        }
                    }
                    break 'resolve; // Exit the loop after resolving the path
                }
                // No signature yet
                None => {
                    match step {
                        SignatureStep::Field { ident, expr } => {
                            self.find_signature(ident);
                        },
                        _ => {}
                    }
                }
            }
        }

        match self.signature {
            Some(sig) => {
                elements.push(ResolvedPathElement::Signature(sig));
            }
            None => {
                // If no signature was found, we can still return the fragments
                for fragment in self.fragments {
                    elements.push(ResolvedPathElement::Error {
                        step: None,
                        kind: LinearError::NoItemInScope {
                            ident: self.expr.to_string(self.db),
                            scope: self.expr.scope_id(self.db),
                        },
                    });
                }
            }
        }

        ResolvedPath { elements }
    }

    fn find_signature(&mut self, identifier: &SpannedIdent) {
        eprintln!(
            "Finding signature for identifier: {:?}",
            identifier.ident.text(self.db)
        );

        // Try variables in scope
        if let Some(variable) = variables_in_scope(self.db, self.file, self.expr.scope_id(self.db))
            .get(&identifier.ident)
        {
            self.signature = Some(signature_for_variable(self.db, *variable));
            return;
        }

        // Try POUs in scope
        if let Some(pou) =
            pous_in_scope(self.db, self.file, self.expr.scope_id(self.db)).get(&identifier.ident)
        {
            self.signature = Some(signature_for_pou(self.db, *pou));
            return;
        }

        // Try namespace resolution
        let path = NamespacePath::from((self.db, &self.fragments));
        let access = NamespaceAccess::new(self.db, Some(path), identifier);
        if let Some(pou) =
            resolve_namespace_access(self.db, self.file, self.expr.scope_id(self.db), access)
        {
            self.signature = Some(signature_for_pou(self.db, pou));
            return;
        }

        // Accumulate fragments if not resolved yet
        self.fragments.push(identifier.clone());
    }
}
