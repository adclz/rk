use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        interned::namespace::NamespaceAccess,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{head::inheritance::inherited_methods, name_res::resolve_namespace_access, ty::Type},
};

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_signature<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Signature<'db> {
    Signature::new(scope).infer_signature(db)
}

impl<'db> Type<'db> {
    pub(crate) fn resolve_spec(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Self {
        match spec.kind(db) {
            SpecKind::Simple(elem) => Type::Elementary(*elem),
            SpecKind::Ref(ref_to) => Type::RefTo(*ref_to),
            SpecKind::Struct(strukt) => Type::Struct(*strukt),
            SpecKind::Array(arr) => Type::Array(*arr),
            SpecKind::ArrayConformand(a) => Type::ArrayConformand(*a),
            SpecKind::Enum(enm) => Type::Enum(*enm),
            SpecKind::Subrange(sub) => Type::SubRange(*sub),
            SpecKind::Target(t) => match resolve_namespace_access(db, &t.path) {
                Some(pou) => Type::new_pou(db, pou),
                None => Type::Never,
            },
        }
    }
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

    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> Signature<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            type_of_specs: FxHashMap::default(),
            namespace_access_to_pou: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    fn infer_signature(mut self, db: &'db dyn WorkspaceDataBase) -> Self {
        if let ScopeKind::Pou(pou) = get_scope(db, self.scope).kind
            && let Pou::DataType(dt) = pou
        {
            self.infer_spec(db, dt.spec(db));
        }

        self.infer_variables(db);
        self.infer_return_type(db);
        self.infer_methods(db);

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
            let _ = self.infer_spec(db, ret_type);
        }
    }

    fn infer_methods(&mut self, db: &'db dyn WorkspaceDataBase) {
        let implementer = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => pou,
            _ => return,
        };

        let declared_methods = &implementer.get_scope_id(db).def_map(db).declared_methods;
        let inherited_methods = inherited_methods(db, implementer);
        for (ns, typ) in &inherited_methods.type_of_namespace_accesses {
            self.namespace_access_to_pou.insert(ns.clone(), *typ);
        }
    }

    fn infer_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        for var in variables {
            let var_type = self.infer_spec(db, var.spec(db));
        }
    }

    fn infer_spec(&mut self, db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Type<'db> {
        let typ = Type::resolve_spec(db, spec);

        match spec.kind(db) {
            SpecKind::Array(arr) => {
                self.infer_spec(db, arr.of_type(db));
            }
            SpecKind::Enum(enm) => {
                if let Some(spec) = enm.typ(db) {
                    self.infer_spec(db, spec);
                }
            }
            SpecKind::Subrange(subrange) => {
                self.infer_spec(db, subrange._type(db));
            }
            SpecKind::Struct(strukt) => {
                for field in &strukt.elements(db) {
                    self.infer_spec(db, field.spec(db));
                }
            }
            SpecKind::Ref(rf) => {
                self.infer_spec(db, *rf);
            }
            _ => {}
        }

        match typ {
            Type::Never => {
                if let SpecKind::Target(target) = spec.kind(db) {
                    self.errors.push(
                        ResolveError::NoNamespaceItemFound {
                            path: target.clone(),
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            Type::Function(_) => {
                self.errors.push(
                    ResolveError::FunctionAsType {
                        expr: spec,
                        ty: typ,
                    }
                    .to_diagnostic(db),
                );
            }
            _ => (),
        };
        self.type_of_specs.insert(spec, typ);
        typ
    }
}
