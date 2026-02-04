use core::panic;

use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        interned::namespace::NamespaceAccess,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::BodyInferenceResult, signature::init_inference::InitExprInferenceResult, ty::Type,
    },
};

pub(crate) mod array;
pub(crate) mod enum_;
pub mod inheritance;
pub mod init_inference;
pub(crate) mod methods;
pub(crate) mod strukt;
pub(crate) mod subrange;
pub(crate) mod usings;
pub(crate) mod variables;

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_signature<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Signature<'db> {
    Signature::new(scope).infer_signature(db)
}

/// Information about an element's position in an array initializer
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub struct ArrayElementPosition {
    /// The dimension this element is in (0 for first dimension, etc.)
    pub dimension: usize,
    /// Number of elements this initializer fills (1 for single values, N for N(value))
    pub count: usize,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    /// Mapping of specs to their inferred types
    pub type_of_specs: FxHashMap<Spec<'db>, Type<'db>>,

    /// Mapping of namespace accesses to their inferred POUs
    pub namespace_access_to_pou: FxHashMap<NamespaceAccess<'db>, Type<'db>>,

    /// Initializer expression inference results
    pub init_expr_result: InitExprInferenceResult<'db>,

    /// BodyInference results (Inference of constant expressions)
    pub body_infer_result: BodyInferenceResult<'db>,

    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> Signature<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            type_of_specs: FxHashMap::default(),
            namespace_access_to_pou: FxHashMap::default(),
            init_expr_result: InitExprInferenceResult::new(scope),
            body_infer_result: BodyInferenceResult::new(scope),
            errors: Vec::new(),
        }
    }

    fn infer_signature(mut self, db: &'db dyn WorkspaceDataBase) -> Self {
        if let ScopeKind::Pou(pou) = get_scope(db, self.scope).kind
            && let Pou::DataType(dt) = pou {
                let typ = Type::new_spec(db, dt.spec(db));
                match dt.spec(db).kind(db) {
                    SpecKind::Array(arr) => {
                        self.infer_array(db, *arr);
                    }
                    SpecKind::Enum(enm) => {
                        self.infer_enum(db, *enm);
                    }
                    SpecKind::Subrange(subrange) => {
                        self.infer_subrange(db, *subrange);
                    }
                    SpecKind::Struct(strukt) => {
                        self.infer_struct(db, *strukt);
                    }
                    SpecKind::Target(target) => {
                        if typ.is_never() {
                            self.errors.push(
                                ResolveError::NoNamespaceItemFound {
                                    path: target.clone(),
                                }
                                .to_diagnostic(db),
                            )
                        };
                    }
                    _ => {}
                }
                self.type_of_specs.insert(dt.spec(db), typ);
                if let Some(expr) = dt.init(db) {
                    self.init_expr_result.resolve_init_expr(
                        db,
                        expr,
                        &mut self.body_infer_result,
                        typ,
                    );
                };
            }

        self.infer_variables(db);
        self.infer_return_type(db);
        self.check_usings(db);
        self.check_methods(db);

        for error in &self.init_expr_result.errors {
            self.errors.push(error.clone());
        }

        for error in &self.body_infer_result.errors {
            self.errors.push(error.clone());
        }

        self
    }

    fn infer_return_type(&mut self, db: &'db dyn WorkspaceDataBase) {
        let return_typ = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.return_type(db).copied(),
                _ => None,
            },
            ScopeKind::MethodDecl(m) => m.return_type(db).copied(),
            _ => None,
        };

        if let Some(ret_type) = return_typ {
            let typ = Type::new_spec(db, ret_type);
            match typ {
                Type::Infer(_) | Type::Never => {
                    if let SpecKind::Target(target) = ret_type.kind(db) {
                        self.errors.push(
                            ResolveError::NoNamespaceItemFound {
                                path: target.clone(),
                            }
                            .to_diagnostic(db),
                        );
                        self.type_of_specs.insert(ret_type, Type::Never);
                    }
                }
                _ => {
                    self.type_of_specs.insert(ret_type, typ);
                }
            }
        }
    }
}
