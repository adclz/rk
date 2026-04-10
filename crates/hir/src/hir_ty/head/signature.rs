use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::{
        config::ConfigResource,
        expressions::spec::{Spec, SpecKind},
        pous::{pou::Pou, variable::VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{
        index_graphs::external_var_lookup,
        resolver::{
            func_call::resolve_params,
            name::{NameResolution, resolve_name},
            visibility::check_test_visibility,
        },
        ty::{CallableType, Type},
    },
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

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    /// Mapping of specs to their inferred types
    pub type_of_specs: FxHashMap<Spec<'db>, Type<'db>>,

    /// USING directives that were used during signature inference (for unused-import linter)
    pub usings_used: FxHashSet<Using<'db>>,

    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> Signature<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            type_of_specs: FxHashMap::default(),
            usings_used: FxHashSet::default(),
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
        self.infer_variables(db);
        self.infer_return_type(db);
        self.infer_access_decls(db);
        self.infer_config_resources(db);
        self.infer_test_cases(db);

        self
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
                    // Resource variables share the config scope but aren't in
                    // ScopeId::variables(), so infer their specs here.
                    for v in r.variables(db).iter() {
                        self.infer_spec(db, v.spec(db));
                    }
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

        // Track USING directives and check test visibility for Target specs
        if let SpecKind::Target(target) = spec.kind(db) {
            let call_site = CallSite::new(spec.scope_id(db), spec.id(db));
            match resolve_name(db, &target.path, spec.scope_id(db)) {
                NameResolution::Pou(pou, using) => {
                    if let Some(using) = using {
                        self.usings_used.insert(using);
                    }
                    check_test_visibility(db, &call_site, pou.get_scope_id(db), &mut self.errors);
                }
                NameResolution::Program(prog) => {
                    check_test_visibility(db, &call_site, prog.scope_id(db), &mut self.errors);
                }
                _ => {}
            }
        }

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
                    // Re-resolve to distinguish ambiguous from not-found
                    match resolve_name(db, &target.path, spec.scope_id(db)) {
                        NameResolution::Ambiguous(candidates) => {
                            self.errors.push(
                                ResolveError::MultipleItemsInScope {
                                    name: target.path.target.ident,
                                    span: spec.get_span(db),
                                    candidates,
                                }
                                .to_diagnostic(db),
                            );
                        }
                        _ => {
                            self.errors.push(
                                ResolveError::NoNamespaceItemFound {
                                    path: target.clone(),
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
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

    fn infer_test_cases(&mut self, db: &'db dyn WorkspaceDataBase) {
        let scope = get_scope(db, self.scope);

        let (pragmas, callable) = match scope.kind {
            ScopeKind::Pou(Pou::Function(f)) => (f.pragmas(db), CallableType::Function(f)),
            _ => return,
        };

        for case in crate::hir_def::pous::pragma::cases(pragmas) {
            resolve_params(db, case, callable, &mut self.errors);
        }
    }
}
