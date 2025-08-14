use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::{InlayHint, InlayHintKind, InlayHintLabel},
};

use crate::{
    hir::{
        expressions::expression::PathExpr,
        interned::{
            identifier::SpannedIdent,
            namespace::{NamespaceAccess, NamespacePath},
        },
        scopes::solver::{pous_in_scope, resolve_namespace_access, variables_in_scope},
        semantic_index::SemanticIndex,
        ty::{ty_for_pou, ty_for_variable, Ty, TyStep, WalkError},
    },
    to_proto::{ToProto},
};

/// Represents a resolved element in a path expression.
/// Contains the expression and its type.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedElement<'db> {
    // The expression that was resolved
    pub expr: PathExpr<'db>,
    // The type of the resolved element
    pub ty: Ty<'db>,
}

impl<'db> ResolvedElement<'db> {
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

impl<'db> ToProto<'db> for ResolvedElement<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.expr.span(db)
    }

    fn inlay_hint(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(self.expr.to_string(db).text(db).to_string()),
            position: self.expr.span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }
}

#[salsa::tracked(returns(ref))]
pub fn resolved_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    expr: PathExpr<'db>,
) -> ResolvedPathResult<'db> {
    ResolvePathExprCtx::new(db, file, expr).resolve_path_expr()
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedPathResult<'db> {
    pub elements: Vec<ResolvedElement<'db>>,
    pub error: Option<WalkError<'db>>,
}

/// [`PathExpr`] resolver.
///
/// Tries to resolve a [`PathExpr`] to a sequence of [`Signature`] steps.
pub struct ResolvePathExprCtx<'db> {
    db: &'db dyn BaseDatabase,
    file: File,
    expr: PathExpr<'db>,
    fragments: Vec<SpannedIdent>,
    signature: Option<Ty<'db>>,
}

impl<'db> ResolvePathExprCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, file: File, expr: PathExpr<'db>) -> Self {
        Self {
            db,
            file,
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
                    //elements.push(sig);

                    for step in steps {
                        match current.linear(self.db, step) {
                            Ok(next_sig) => {
                                current = next_sig;
                                elements.push(ResolvedElement::new(*step.get_expr(), current));
                            }
                            Err(error) => {
                                return ResolvedPathResult {
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
                elements.push(ResolvedElement::new(self.expr, sig));
            }
            None => {
                return ResolvedPathResult {
                    elements,
                    error: Some(WalkError::NoItemInScope {
                        expr: self.expr,
                        scope: self.expr.scope_id(self.db),
                    }),
                };
            }
        }

        ResolvedPathResult {
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
