use db::WorkspaceDataBase;

pub mod func_call;
pub mod invocation;
pub mod visibility;
pub mod walk;

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::{
        expressions::expression::{
            BeginPathExpr, MultibitsPart, PathExpr, VariableAccess, VariableAccessKind,
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::BodyInferenceResult, head::signature::infer_signature,
        name_res::resolve_namespace_access, resolver::walk::PathPlaceBuilder, ty::Type,
    },
};

#[derive(Debug, Copy, Clone)]
pub struct Resolver<'db> {
    pub root: PathResolutionRoot<'db>,
}

#[derive(Debug, Copy, Clone)]
pub enum PathResolutionRoot<'db> {
    /// Path starts from a known value/type (THIS, SUPER, implicit self)
    Value { base: Type<'db> },

    /// Path starts from a namespace / scope
    Namespace { scope: ScopeId<'db> },
}

impl<'db> Resolver<'db> {
    pub fn for_scope(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Self {
        let root = match get_scope(db, scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::DataType(_) => PathResolutionRoot::Namespace { scope },
                _ => PathResolutionRoot::Value {
                    base: Type::new_pou(db, pou),
                },
            },
            ScopeKind::MethodDecl(m) => PathResolutionRoot::Value {
                base: Type::MethodDecl(m.into()),
            },
            ScopeKind::Program(program) => PathResolutionRoot::Value {
                base: Type::Program(program),
            },
            _ => PathResolutionRoot::Namespace { scope },
        };

        Self { root }
    }

    /// Try to resolve `path_expr` as a fully-qualified namespace access.
    ///
    /// Returns `true` if the path was successfully resolved, `false` otherwise
    /// (an error diagnostic is always pushed on failure).
    fn try_resolve_as_fq(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> bool {
        let Some((access, _)) = path_expr.to_namespace_access(db) else {
            ctx.errors.push(
                ResolveError::NoItemInScope {
                    expr: path_expr,
                    scope: path_expr.scope_id(db),
                }
                .to_diagnostic(db),
            );
            return false;
        };

        // Methods declarations, just like FUNCTIONS, can reference themselves (return type)
        // but the resolve_namespace_access only searches for POUs,
        // so we also check if the target matches the name of a method in the current scope
        // todo: move this logic inside resolve_namespace_access and make it more robust (handle shadowing, etc.)
        match get_scope(db, path_expr.get_scope_id(db)).kind {
            ScopeKind::MethodDecl(method) => {
                // todo: check shadowing
                if access.target == method.name(db) {
                    ctx.type_of_path_expr
                        .insert(path_expr, Type::MethodDecl(method.into()));
                    return true;
                }
            }
            _ => (),
        }

        // Generics
        let signature = infer_signature(db, path_expr.scope_id(db));
        if let Some(generic_type) = signature.type_of_generic.get(&access.target) {
            ctx.type_of_path_expr
                .insert(path_expr, generic_type.clone());
            return true;
        }

        match resolve_namespace_access(db, access) {
            Some(pou) => {
                ctx.type_of_path_expr
                    .insert(path_expr, Type::new_pou(db, pou));
                true
            }
            None => {
                ctx.errors.push(
                    ResolveError::NoItemInScope {
                        expr: path_expr,
                        scope: path_expr.scope_id(db),
                    }
                    .to_diagnostic(db),
                );
                false
            }
        }
    }

    pub fn resolve_variable_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match var_access.kind(db) {
            VariableAccessKind::Direct(dv) => {
                ctx.type_of_direct_variable
                    .insert(dv, Type::DirectVariable((dv, var_access.multibits(db))));
            }
            VariableAccessKind::Symbolic(s) => {
                self.resolve_begin_path_expr(db, s, var_access.multibits(db), ctx);
            }
        }
    }

    pub fn resolve_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: BeginPathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match self.root {
            PathResolutionRoot::Value { base } => {
                base.walk_begin_path_expr(db, path_expr, multibits, ctx);
            }
            PathResolutionRoot::Namespace { .. } => {
                // a begin path expr will always refer to a local variable in this context
            }
        };
    }

    pub fn resolve_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match self.root {
            PathResolutionRoot::Value { base } => {
                self.resolve_path_steps(base, db, path_expr, multibits, ctx)
            }
            PathResolutionRoot::Namespace { .. } => {
                self.try_resolve_as_fq(db, path_expr, ctx);
            }
        }
    }

    /// Walk each step of a path expression against the current type.
    ///
    /// On the **first** step, errors are suppressed because a failed local
    /// lookup may still succeed as a fully-qualified namespace access.
    /// If step 0 fails and the root is a `Value`, we fall back to FQ resolution.
    pub(crate) fn resolve_path_steps(
        &self,
        mut current: Type<'db>,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let steps = path_expr.flatten(db);
        let Some(first_step) = steps.first() else {
            return;
        };

        let mut place = PathPlaceBuilder {
            current_typ: current,
            current_path: first_step.get_expr(db),
        };

        for (index, step) in steps.iter().enumerate() {
            let is_first_step = index == 0;

            // Suppress errors on the first step: if it fails we may fall back to FQ resolution.
            current.walk_path_expr(db, !is_first_step, step, multibits, &mut place, ctx);

            // Check whether walk_path_expr actually resolved this step.
            let resolved = ctx.type_of_path_expr.contains_key(&step.get_expr(db));

            if !resolved {
                if is_first_step && matches!(self.root, PathResolutionRoot::Value { .. }) {
                    self.try_resolve_as_fq(db, path_expr, ctx);
                }
                return;
            }

            current = ctx.type_of_path_expr_with_adjustments(step.get_expr(db));
        }
    }
}
