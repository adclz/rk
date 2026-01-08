use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{body_inference::TypeError, init_inference::InitInferenceError},
    hir_def::{
        expressions::{
            expression::{InitExpr, InitExprKind},
            spec::Spec,
        },
        interned::identifier::SpanIdent,
        pous::{data_type::DataType, variable::VariableDecl},
        scope::ScopeId,
    },
    hir_ty::{
        body_inference::BodyInferenceResult, def_map::FxIndexMap, expr_store::InitExprWalkStep,
        infer::expr::InferExprCtx, resolver::Resolver, ty::Type,
    },
};

#[salsa::tracked(returns(ref), no_eq)]
pub fn infer_variable<'db>(
    db: &'db dyn BaseDatabase,
    variable: VariableDecl<'db>,
) -> InitExprInferenceResult<'db> {
    let typ = Type::new_spec(db, variable.spec(db));
    let mut infer = InitExprInferenceResult::new(db, variable.scope_id(db));
    if let Some(expr) = variable.init(db) {
        infer.resolve_init_expr(db, expr, typ);
    };
    infer
}

#[salsa::tracked(returns(ref), no_eq)]
pub fn infer_data_type<'db>(
    db: &'db dyn BaseDatabase,
    data_type: DataType<'db>,
) -> InitExprInferenceResult<'db> {
    let typ = Type::new_spec(db, data_type.spec(db));
    let mut infer = InitExprInferenceResult::new(db, data_type.scope_id(db));
    if let Some(expr) = data_type.init(db) {
        infer.resolve_init_expr(db, expr, typ);
    };
    infer
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct InitExprInferenceResult<'db> {
    /// Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,
    /// Mapping of init expr to their resolved types
    pub type_of_expr: FxHashMap<InitExpr<'db>, Type<'db>>,
    /// Mapping of types to their spec declarations
    pub type_definitions: FxHashMap<Type<'db>, Spec<'db>>,
    /// BodyInference results
    pub body_infer_result: BodyInferenceResult<'db>,
    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> InitExprInferenceResult<'db> {
    pub fn new(db: &'db dyn BaseDatabase, scope: ScopeId<'db>) -> Self {
        Self {
            type_of_expr: FxHashMap::default(),
            scope,
            type_definitions: FxHashMap::default(),
            body_infer_result: BodyInferenceResult::new(scope),
            errors: Vec::new(),
        }
    }

    pub fn resolve_init_expr(
        &mut self,
        db: &'db dyn BaseDatabase,
        expr: InitExpr<'db>,
        typ: Type<'db>,
    ) {
        let map = expr.flatten(db);
        self.resolve_expr(db, expr, typ, &map);
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

        self.type_of_expr.insert(expr, narrowed);

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

            InitExprKind::ConstantExpr(rhs) => {
                let mut infer_ctx = InferExprCtx::new(Resolver::for_scope(db, self.scope));
                infer_ctx.resolve_expr(db, rhs, &mut self.body_infer_result);
                infer_ctx.check_expr(db, rhs, &mut self.body_infer_result);

                if let Err(err) =
                    infer_ctx.coerce_type_with_expr(db, expected, rhs, &mut self.body_infer_result)
                {
                    self.errors.push(err.into_non_assignable(
                        db,
                        expected,
                        CallSite::from_init_expr(db, expr),
                    ));
                }

                narrowed
            }
        }
    }
}
