use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError, e3_type::TypeError,
    },
    hir_def::{
        expressions::spec::{ElementarySpec, Spec, SpecKind},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        pous::{
            generics::{AnyGeneric, GenericParam},
            pou::Pou,
        },
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
    // todo: this belongs in the resolver module
    pub(crate) fn resolve_spec(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Self {
        match spec.kind(db) {
            SpecKind::Simple(elem) => Type::Elementary(*elem),
            SpecKind::Ref(ref_to) => Type::RefTo(*ref_to),
            SpecKind::Struct(strukt) => Type::Struct(*strukt),
            SpecKind::Array(arr) => Type::Array(*arr),
            SpecKind::ArrayConformand(a) => Type::ArrayConformand(*a),
            SpecKind::Enum(enm) => Type::Enum(*enm),
            SpecKind::Subrange(sub) => Type::SubRange(*sub),
            SpecKind::Target(t) => {
                let generics = spec.scope_id(db).generics(db);

                if let Some(generics) = generics {
                    for generic in generics.iter() {
                        if generic.name(db) == *t.path.target {
                            return Type::Generic(*generic);
                        }
                    }
                }

                match resolve_namespace_access(db, &t.path) {
                    Some(pou) => Type::new_pou(db, pou),
                    None => Type::Never,
                }
            }
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Constraint<'db> {
    GenericParameter(Ident),
    AnyGeneric(AnyGeneric),
    Spec(Spec<'db>),
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    /// Mapping of specs to their inferred types
    pub type_of_specs: FxHashMap<Spec<'db>, Type<'db>>,

    /// Mapping of namespace accesses to their inferred POUs
    pub namespace_access_to_type: FxHashMap<NamespaceAccess<'db>, Type<'db>>,

    /// Mapping of generic parameters to their inferred types (for generics declared on this POU)
    pub type_of_generic: FxHashMap<Ident, Type<'db>>,

    //// Mapping of generic parameters to their spec constraints (for generics declared on this POU)
    pub constraint_of_generic: FxHashMap<Ident, Vec<Constraint<'db>>>,

    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> Signature<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            type_of_specs: FxHashMap::default(),
            namespace_access_to_type: FxHashMap::default(),
            type_of_generic: FxHashMap::default(),
            constraint_of_generic: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    fn infer_signature(mut self, db: &'db dyn WorkspaceDataBase) -> Self {
        if let ScopeKind::Pou(pou) = get_scope(db, self.scope).kind
            && let Pou::DataType(dt) = pou
        {
            self.infer_spec(db, dt.spec(db));
        }

        self.infer_generics(db);
        self.infer_variables(db);
        self.infer_return_type(db);
        self.infer_methods(db);

        self
    }

    fn infer_generics(&mut self, db: &'db dyn WorkspaceDataBase) {
        let generics = match self.scope.generics(db) {
            Some(generics) => generics,
            None => return,
        };
        
        let generics_hashmap = &self.scope.def_map(db).generics;

        for generic in generics {
            if generic.as_builtin_generic(db).is_none() {
                self.errors
                    .push(TypeError::InvalidGenericType { param: *generic }.to_diagnostic(db));
            }

            for constraint in generic.spec_constraints(db) {
                match constraint.spec.kind(db) {
                    SpecKind::Simple(elementary) => {
                        if elementary.is_simple() {
                            self.constraint_of_generic
                                .entry(generic.name(db))
                                .or_default()
                                .push(Constraint::Spec(constraint.spec));
                        } else {
                            self.errors.push(
                                TypeError::InvalidGenericConstraint {
                                    param: *generic,
                                    constraint: constraint.spec,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                    SpecKind::Target(target) => {
                        // can not create a generic constraint to a namespace item.
                        // todo: allowing this means we should add support for subtyping
                        if target.path.namespace.is_some() {
                            self.errors.push(
                                TypeError::InvalidGenericConstraint {
                                    param: *generic,
                                    constraint: constraint.spec,
                                }
                                .to_diagnostic(db),
                            );
                        } else {
                            let target = target.path.target;
                            if let Some(generic) = generics_hashmap.get(&target) {
                                // refer to a locally declared generic parameter
                                self.constraint_of_generic
                                    .entry(generic.name(db))
                                    .or_default()
                                    .push(Constraint::GenericParameter(generic.name(db)));
                            } else if let Some(any) = AnyGeneric::is_builtin_any(db, &target) {
                                // refer to a builtin generic parameter
                                self.constraint_of_generic
                                    .entry(generic.name(db))
                                    .or_default()
                                    .push(Constraint::AnyGeneric(any));
                            } else {
                                self.errors.push(
                                    TypeError::InvalidGenericConstraint {
                                        param: *generic,
                                        constraint: constraint.spec,
                                    }
                                    .to_diagnostic(db),
                                );
                            }
                        }
                    }
                    _ => {
                        self.errors.push(
                            TypeError::InvalidGenericConstraint {
                                param: *generic,
                                constraint: constraint.spec,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
            }
        }
    }

    fn infer_return_type(&mut self, db: &'db dyn WorkspaceDataBase) {
        let return_typ = self.scope.return_type(db);

        if let Some(ret_type) = return_typ {
            let _ = self.infer_spec(db, *ret_type);
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
            self.namespace_access_to_type.insert(ns.clone(), *typ);
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
