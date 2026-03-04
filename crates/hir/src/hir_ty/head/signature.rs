use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError, e3_type::TypeError},
    hir_def::{
        config::ConfigResource,
        expressions::spec::{Spec, SpecKind},
        interned::identifier::Ident,
        pous::{generics::AnyGeneric, pou::Pou, variable::VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{index_graphs::external_var_lookup, ty::Type},
};

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Constraint<'db> {
    /// Main type bound (e.g., `T: ANY_INT`)
    TypeBound(AnyGeneric),
    /// INTO<OtherGenericParam> constraint
    GenericParameter(Ident),
    /// INTO<ANY_*> constraint (e.g., INTO<ANY_INT>)
    AnyGeneric(AnyGeneric),
    /// INTO<ConcreteType> constraint (e.g., INTO<INT>)
    Spec(Spec<'db>),
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    /// Mapping of specs to their inferred types
    pub type_of_specs: FxHashMap<Spec<'db>, Type<'db>>,

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

        self.infer_extends_implements(db);
        self.infer_generics(db);
        self.infer_variables(db);
        self.infer_return_type(db);
        self.infer_access_decls(db);
        self.infer_config_resources(db);

        self
    }

    fn infer_generics(&mut self, db: &'db dyn WorkspaceDataBase) {
        let generics = match self.scope.generics(db) {
            Some(generics) => generics,
            None => return,
        };

        let generics_hashmap = &self.scope.def_map(db).generics;

        for generic in generics {
            let builtin = generic.as_builtin_generic(db);
            if builtin.is_none() {
                self.errors
                    .push(TypeError::InvalidGenericType { param: *generic }.to_diagnostic(db));
            }

            // Store the main type bound (e.g., ANY_INT from `T: ANY_INT`)
            if let Some(any) = builtin {
                self.constraint_of_generic
                    .entry(generic.name(db))
                    .or_default()
                    .push(Constraint::TypeBound(any));
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

    fn infer_extends_implements(&mut self, db: &'db dyn WorkspaceDataBase) {
        let pou = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => pou,
            _ => return,
        };

        match pou {
            Pou::Class(class) => {
                if let Some(extends) = class.extends(db) {
                    self.infer_spec(db, *extends);
                }
                for iface in class.implements(db) {
                    self.infer_spec(db, *iface);
                }
            }
            Pou::FunctionBlock(fb) => {
                if let Some(extends) = fb.extends(db) {
                    self.infer_spec(db, *extends);
                }
                for iface in fb.implements(db) {
                    self.infer_spec(db, *iface);
                }
            }
            Pou::Interface(iface) => {
                if let Some(extends) = iface.extends(db) {
                    for spec in extends {
                        self.infer_spec(db, *spec);
                    }
                }
            }
            _ => {}
        }
    }

    fn infer_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        for var in variables {
            let typ_of_var = self.infer_spec(db, var.spec(db));

            if var.kind(db) == VariableKind::External {
                let var_name = var.get_name_ident(db);
                if external_var_lookup(db, var_name).is_none() {
                    self.errors
                        .push(ResolveError::ExternalVarNotFound { var: *var }.to_diagnostic(db));
                }
            }
        }
    }

    fn infer_config_resources(&mut self, db: &'db dyn WorkspaceDataBase) {
        let config = match get_scope(db, self.scope).kind {
            ScopeKind::Config(c) => c,
            _ => return,
        };

        for res in config.resources(db).iter() {
            match res {
                ConfigResource::Program(p) => {
                    self.infer_spec(db, p.prog_type(db));
                }
                ConfigResource::Resource(r) => {
                    for p in r.programs(db).iter() {
                        self.infer_spec(db, p.prog_type(db));
                    }
                }
                ConfigResource::Task(_) => {}
            }
        }
    }

    fn infer_access_decls(&mut self, db: &'db dyn WorkspaceDataBase) {
        let program = match get_scope(db, self.scope).kind {
            ScopeKind::Program(prog) => prog,
            _ => return,
        };

        let def_map = self.scope.def_map(db);

        for decl in program.prog_access_decls(db) {
            let declared_ty = self.infer_spec(db, decl.spec);

            // Look up the referenced variable in the program's scope
            let var_name = decl.variable.ident(db).ident;
            match def_map.global_variables.get(&var_name) {
                Some(var) => {
                    // Variable found - compare declared spec type with actual variable type
                    let var_ty = self
                        .type_of_specs
                        .get(&var.spec(db))
                        .copied()
                        .unwrap_or(Type::Never);
                    if declared_ty != Type::Never && var_ty != Type::Never && declared_ty != var_ty
                    {
                        self.errors.push(
                            ResolveError::AccessDeclTypeMismatch {
                                var_origin: *var,
                                spec: decl.spec,
                                expected: declared_ty,
                                actual: var_ty,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
                None => {
                    self.errors.push(
                        ResolveError::NoItemInScope {
                            expr: decl.variable,
                            scope: self.scope,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
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
