use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

pub mod walk;
pub mod visibility;
pub mod func_call;
pub mod invocation;
pub mod body;

use crate::{
    CallSite, HasVisibility, HirNodeInfo, Visibility,
    check::errors::{
        analysis_error::ToIdeDiagnostic, body_inference::BodyInferenceError,
        init_inference::InitInferenceError, visibility::VisibilityError,
    },
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, InitExpr, PathExpr, VariableAccess, VariableAccessKind},
            invocation::InvocationKind,
        },
        namespace::NamespaceDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        body_inference::{Adjustment, BodyInferenceResult},
        expr_store::{InitExprWalkStep, PathExprWalkStep},
        inheritance_solver::inherited_methods,
        init_inference::InitExprInferenceResult,
        name_res::resolve_namespace_access,
        ty::Type,
    },
};

#[derive(Debug, Copy, Clone)]
pub struct Resolver<'db> {
    pub scope: ScopeId<'db>,
    pub walkable_typ: Option<Type<'db>>,
}

impl<'db> Resolver<'db> {
    pub fn new(scope: ScopeId<'db>, walkable_typ: Option<Type<'db>>) -> Self {
        Self { scope, walkable_typ }
    }

    fn resolve_as_fq(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let Some((access, _)) = path_expr.to_namespace_access(db) else {
            infer_results.errors.push(
                BodyInferenceError::NoItemInScope {
                    expr: path_expr,
                    scope: path_expr.scope_id(db),
                }
                .to_diagnostic(db),
            );
            return Type::Never;
        };

        match resolve_namespace_access(db, &access) {
            Some(pou) => {
                let typ = Type::new_pou(db, pou);
                infer_results.type_of_path_expr.insert(path_expr, typ);
                typ
            }
            None => {
                infer_results.errors.push(
                    BodyInferenceError::NoItemInScope {
                        expr: path_expr,
                        scope: path_expr.scope_id(db),
                    }
                    .to_diagnostic(db),
                );
                Type::Never
            }
        }
    }

    #[must_use]
    pub fn resolve_variable_access(
        &self,
        db: &'db dyn BaseDatabase,
        var_access: VariableAccess<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match var_access.kind(db) {
            VariableAccessKind::Direct { .. } => todo!(),
            VariableAccessKind::Symbolic(s) => match self.walkable_typ {
                Some(typ) => typ.walk_begin_path_expr(db, s, infer_results),
                None => Type::Never,
            },
        }
    }

    #[must_use]
    pub fn resolve_begin_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: BeginPathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match self.walkable_typ {
            Some(typ) => typ.walk_begin_path_expr(db, path_expr, infer_results),
            None => Type::Never,
        }
    }

    #[must_use]
    pub fn resolve_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match self.walkable_typ {
            Some(start) => self.resolve_path_steps(start, db, path_expr, infer_results),
            None => self.resolve_as_fq(db, path_expr, infer_results),
        }
    }

    #[must_use]
    fn resolve_path_steps(
        &self,
        mut current: Type<'db>,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let steps = path_expr.flatten(db);
        for (index, step) in steps.into_iter().enumerate() {
            match current.walk_path_expr(db, index != 0, step, ctx) {
                Type::Never => {
                    // no path was resolved yet
                    if index == 0 {
                        return self.resolve_as_fq(db, path_expr, ctx);
                    }
                    return Type::Never;
                }
                next => current = next,
            }
        }
        current
    }
}
