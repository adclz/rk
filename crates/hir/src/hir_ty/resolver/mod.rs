use db::WorkspaceDataBase;

pub mod body;
pub mod func_call;
pub mod invocation;
pub mod visibility;
pub mod walk;

use crate::{
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
        body_inference::BodyInferenceResult, name_res::resolve_namespace_access,
        resolver::walk::PlaceBuilder, ty::Type,
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

    fn allows_fq_fallback(&self) -> bool {
        matches!(self.root, PathResolutionRoot::Value { .. })
    }

    fn resolve_as_fq(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) {
        let kind = path_expr.expr(db);
        let Some((access, _)) = path_expr.to_namespace_access(db) else {
            infer_results.errors.push(
                ResolveError::NoItemInScope {
                    expr: path_expr,
                    scope: path_expr.scope_id(db),
                }
                .to_diagnostic(db),
            );
            return;
        };

        match resolve_namespace_access(db, access) {
            Some(pou) => {
                infer_results
                    .type_of_path_expr
                    .insert(path_expr, Type::new_pou(db, pou));
            }
            None => {
                infer_results.errors.push(
                    ResolveError::NoItemInScope {
                        expr: path_expr,
                        scope: path_expr.scope_id(db),
                    }
                    .to_diagnostic(db),
                );
            }
        }
    }

    pub fn resolve_variable_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) {
        match var_access.kind(db) {
            VariableAccessKind::Direct(dv) => {
                infer_results
                    .type_of_direct_variable
                    .insert(dv, Type::DirectVariable((dv, var_access.multibits(db))));
            }
            VariableAccessKind::Symbolic(s) => {
                self.resolve_begin_path_expr(db, s, var_access.multibits(db), infer_results);
            }
        }
    }

    pub fn resolve_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: BeginPathExpr<'db>,
        multibits: Option<MultibitsPart>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) {
        match self.root {
            PathResolutionRoot::Value { base } => {
                base.walk_begin_path_expr(db, path_expr, multibits, infer_results);
            }
            PathResolutionRoot::Namespace { scope } => {
                // a begin path expr will always refer to a local variable in this context
            }
        };
    }

    pub fn resolve_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) {
        match self.root {
            PathResolutionRoot::Value { base } => {
                self.resolve_path_steps(base, db, path_expr, multibits, infer_results)
            }
            PathResolutionRoot::Namespace { scope } => {
                self.resolve_as_fq(db, path_expr, infer_results)
            }
        }
    }

    fn resolve_path_steps(
        &self,
        mut current: Type<'db>,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let steps = path_expr.flatten(db);
        let mut place = PlaceBuilder {
            current_typ: current,
            current_path: match steps.first() {
                Some(step) => *step.get_expr(),
                None => return,
            },
        };

        for (index, step) in steps.iter().enumerate() {
            current.walk_path_expr(db, index != 0, step, multibits, &mut place, ctx);

            if ctx
                .type_of_path_expr
                .get(step.get_expr())
                .copied()
                .unwrap_or_default()
                .is_never()
            {
                // Fallback ONLY if root allows it
                if index == 0 && self.allows_fq_fallback() {
                    self.resolve_as_fq(db, path_expr, ctx);
                }

                return;
            }

            current = ctx
                .type_of_path_expr_with_adjustments(*step.get_expr())
                .unwrap_or_default();
        }
    }
}
