use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    check::errors::{body_inference::TypeError, init_inference::InitInferenceError},
    hir_def::{
        expressions::{
            expression::{InitExpr, InitExprKind},
            spec::Spec,
        },
        interned::identifier::SpanIdent,
        scope::ScopeId,
    },
    hir_ty::{
        body_inference::BodyInferenceResult, def_map::FxIndexMap, expr_store::InitExprWalkStep, infer::expr::InferExprCtx, resolver::Resolver, ty::Type
    },
};

#[salsa::tracked(returns(ref))]
pub fn infer_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    typ: Type<'db>,
    init_expr: InitExpr<'db>,
) -> InitExprInferenceResult<'db> {
    let mut result = InitExprInferenceResult::new(db, init_expr);

    result.resolve_init_expr(db, typ);
    result
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct InitExprInferenceResult<'db> {
    /// Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,
    /// The init expression being inferred
    pub expr: InitExpr<'db>,
    /// Mapping of init expr to their resolved types
    pub type_of_expr: FxHashMap<InitExpr<'db>, Type<'db>>,
    /// Mapping of types to their spec declarations
    pub type_definitions: FxHashMap<Type<'db>, Spec<'db>>,
    /// BodyInference results
    pub body_infer_result: BodyInferenceResult<'db>,
    /// Errors encountered during inference
    pub errors: Vec<InitInferenceError<'db>>,
}

impl<'db> InitExprInferenceResult<'db> {
    pub fn new(db: &'db dyn BaseDatabase, init_expr: InitExpr<'db>) -> Self {
        Self {
            type_of_expr: FxHashMap::default(),
            expr: init_expr,
            scope: init_expr.get_scope_id(db),
            type_definitions: FxHashMap::default(),
            body_infer_result: BodyInferenceResult::new(init_expr.get_scope_id(db)),
            errors: Vec::new(),
        }
    }

    pub fn resolve_init_expr(&mut self, db: &'db dyn BaseDatabase, typ: Type<'db>) {
        let map = self.expr.flatten(db);
        self.resolve_expr(db, self.expr, typ, &map);
    }

    fn resolve_expr(
        &mut self,
        db: &'db dyn BaseDatabase,
        expr: InitExpr<'db>,
        expected: Type<'db>,
        map: &FxIndexMap<InitExpr<'db>, InitExprWalkStep<'db>>,
    ) -> Type<'db> {
        // Lookup walk step (from flatten)
        let step = map.get(&expr).copied();

        let narrowed = match step {
            Some(s) => expected.walk_init_expr(db, expr, s, self),
            None => expected,
        };

        match expr.kind(db) {
            InitExprKind::StructInit { values } => {
                for v in values {
                    self.resolve_expr(db, v, narrowed, map);
                }
                narrowed
            }

            InitExprKind::StructElement { value, .. } => {
                self.resolve_expr(db, *value, narrowed, map)
            }

            InitExprKind::ArrayInit { values } => {
                for v in values {
                    self.resolve_expr(db, v, narrowed, map);
                }
                narrowed
            }

            InitExprKind::ArrayIndexedElement { values, .. } => {
                for v in values {
                    self.resolve_expr(db, v, narrowed, map);
                }
                narrowed
            }

            InitExprKind::ConstantExpr(e) => {
                let inferred = InferExprCtx::new(self.scope, Resolver::new(Some(expected)))
                    .infer_expr(db, e, &mut self.body_infer_result);

                if !expected.coerce_with(db, inferred, self.scope) {
                    self.errors
                        .push(InitInferenceError::TypeMismatch(TypeError::NotAssignable {
                            target: expected,
                            value: inferred,
                            expr: e.into(),
                        }));
                }

                inferred
            }
        }
    }
}
