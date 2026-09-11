use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::errors::e03_type::TypeError;
use crate::check::errors::e14_config::ConfigError;
use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e02_resolve::ResolveError, e11_oop::OopError},
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        pous::{function::Function, interface::Interface, pou::Pou, variable::VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{
        index_graphs::{external_var_lookup, namespace_pou_candidates, pou_candidates},
        resolver::{
            name::{NameResolution, enclosing_namespace_path, resolve_name},
            visibility::{check_namespace_visibility, check_test_visibility},
        },
        ty::Type,
    },
};

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_signature<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Signature<'db> {
    Signature::new(scope).infer_signature(db)
}

/// The signature of a FUNCTION — what identifies a declaration among
/// same-named ones. Equal signatures are a duplicate (E0102); a difference
/// anywhere, params or return, makes a legal overload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature<'db> {
    /// The ordered types of the parameters a caller supplies positionally:
    /// `VAR_INPUT` and `VAR_IN_OUT` (pure `VAR_OUTPUT` `=>` bindings don't
    /// participate in selection, so they're excluded). What argument matching
    /// selects an overload by.
    pub params: Vec<Type<'db>>,
    /// The normalized return type, `None` for a void FUNCTION. What a
    /// RETURN-directed overload set (same params) is picked by.
    pub ret: Option<Type<'db>>,
}

/// Every FUNCTION the call could name: the declarations `name` resolves to,
/// in the namespace the callee was found in. What overload resolution picks
/// from, and what signature help lists so an overloaded name shows every
/// candidate rather than whichever one resolution landed on.
pub fn overload_set<'db>(
    db: &'db dyn WorkspaceDataBase,
    first: Function<'db>,
) -> Vec<Function<'db>> {
    let name = first.name(db);
    let candidates = match enclosing_namespace_path(db, first.scope_id(db)) {
        Some(path) => namespace_pou_candidates(db, path, name),
        None => pou_candidates(db, name),
    };
    let mut functions: Vec<Function<'db>> = candidates
        .into_iter()
        .filter_map(|p| match p {
            Pou::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    // A TRUE duplicate (same params, same return) is E0102 at the
    // declaration; the call resolves against the surviving first as if the
    // twin did not exist — one error, not ambiguity noise on every call.
    let mut seen: Vec<FunctionSignature<'db>> = Vec::new();
    functions.retain(|f| {
        let key = function_signature(db, *f);
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    functions
}

/// The [`FunctionSignature`] of `f`. It reads the already-inferred head types
/// (`infer_signature`) rather than re-resolving, so it is a pure consumer of
/// head inference.
///
/// CYCLE HAZARD: never call this from within `infer_signature` (or anything it
/// transitively runs). Gathering another function's signature while a signature
/// is being built re-enters the head query and salsa-cycles. Only out-of-head
/// consumers — the duplicate check, overload resolution, MIR symbol mangling —
/// may call it.
pub fn function_signature<'db>(
    db: &'db dyn WorkspaceDataBase,
    f: Function<'db>,
) -> FunctionSignature<'db> {
    use crate::hir_ty::infer::Infer;
    let sig = infer_signature(db, f.scope_id(db));
    let params = f
        .variables(db)
        .iter()
        .filter(|v| matches!(v.kind(db), VariableKind::Input | VariableKind::InOut))
        .map(|v| {
            sig.type_of_specs
                .get(&v.spec(db))
                .copied()
                .unwrap_or(Type::Never)
                .normalize(db)
        })
        .collect();
    let ret = f.return_type(db).map(|spec| spec.infer(db).normalize(db));
    FunctionSignature { params, ret }
}

/// The minimum number of positional arguments a call to `f` must supply: the
/// count of VAR_INPUT/VAR_IN_OUT params that are *required* — VAR_IN_OUT, or
/// VAR_INPUT with no constant default. Trailing params with a constant default
/// may be omitted. A call is viable for this overload iff
/// `required <= arg_count <= params.len()`.
pub fn function_required_arity<'db>(db: &'db dyn WorkspaceDataBase, f: Function<'db>) -> usize {
    use crate::hir_def::expressions::expression::InitExprKind;
    f.variables(db)
        .iter()
        .filter(|v| match v.kind(db) {
            VariableKind::InOut => true,
            VariableKind::Input => match v.init(db) {
                Some(init) => !matches!(init.kind(db), InitExprKind::ConstantExpr(_)),
                None => true,
            },
            _ => false,
        })
        .count()
}

/// Find an interface reachable at the leaf of a spec — directly, or through an
/// array element / reference target (e.g. `ARRAY OF ITF1`, `REF_TO ITF1`). Used
/// to reject interface types outside VAR_INPUT / VAR_IN_OUT parameters.
fn spec_interface<'db>(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Option<Interface<'db>> {
    match spec.kind(db) {
        SpecKind::Array(arr) => spec_interface(db, arr.of_type(db)),
        SpecKind::Ref(rf) => spec_interface(db, *rf),
        _ => match Type::resolve_spec(db, spec) {
            Type::Interface(i) => Some(i),
            _ => None,
        },
    }
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

        self
    }

    fn infer_return_type(&mut self, db: &'db dyn WorkspaceDataBase) {
        let return_typ = self.scope.return_type(db);

        if let Some(ret_type) = return_typ {
            let _ = self.infer_spec(db, *ret_type);

            // Design 1: an interface may not be a return type — that would flow
            // the concrete type callee→caller, which can't be monomorphized.
            if let Some(interface) = spec_interface(db, *ret_type) {
                self.errors.push(
                    OopError::InterfaceNotAllowedInReturn {
                        interface,
                        spec: *ret_type,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
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
            let _ = self.infer_spec(db, var.spec(db));

            // An ABSTRACT type has no implementation of its own, so a
            // variable of it is an instance that cannot answer its own
            // methods. A REF_TO one is fine — that is a reference to some
            // derived instance, not an instance.
            if let Type::FunctionBlock(fb) = Type::resolve_spec(db, var.spec(db))
                && fb.modifier(db).contains(crate::Modifier::ABSTRACT)
            {
                self.errors.push(
                    OopError::InstantiatedAbstractPou {
                        var: *var,
                        pou: Pou::FunctionBlock(fb),
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            if let Type::Class(cl) = Type::resolve_spec(db, var.spec(db))
                && cl.modifier(db).contains(crate::Modifier::ABSTRACT)
            {
                self.errors.push(
                    OopError::InstantiatedAbstractPou {
                        var: *var,
                        pou: Pou::Class(cl),
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }

            // Design 1 (params-only): a DIRECT interface is allowed only as a
            // VAR_INPUT / VAR_IN_OUT parameter (it monomorphizes to a concrete
            // type); elsewhere it is E1121. A NESTED interface (array element,
            // ref target — e.g. `ARRAY OF ITF1`, `REF_TO ITF1`) has no valid
            // placement at all, not even as a param, so it is E1123.
            if let Some(interface) = spec_interface(db, var.spec(db)) {
                if matches!(Type::resolve_spec(db, var.spec(db)), Type::Interface(_)) {
                    if !matches!(var.kind(db), VariableKind::Input | VariableKind::InOut) {
                        self.errors.push(
                            OopError::InterfaceOnlyAllowedAsParam {
                                var: *var,
                                interface,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    } else if let Some(pou_kind) = match get_scope(db, self.scope).kind {
                        // An input on a POU with instance state is STORED
                        // across calls, so it is not a parameter in the
                        // params-only design; this shape used to pass
                        // `rk check` and ICE in `rk compile`.
                        ScopeKind::Pou(Pou::FunctionBlock(_)) => Some("FUNCTION_BLOCK"),
                        ScopeKind::Pou(Pou::Class(_)) => Some("CLASS"),
                        ScopeKind::Program(_) => Some("PROGRAM"),
                        _ => None,
                    } {
                        self.errors.push(
                            OopError::InterfaceParamOnStatefulPou {
                                var: *var,
                                interface,
                                pou_kind,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                } else {
                    self.errors.push(
                        OopError::InterfaceNotAllowedNested {
                            interface,
                            spec: var.spec(db),
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }

            if var.kind(db) == VariableKind::External {
                let var_name = var.get_name_ident(db);
                if external_var_lookup(db, var_name).is_none() {
                    self.errors.push(
                        ResolveError::ExternalVarNotFound { var: *var }
                            .to_diagnostic(db, self.scope.file(db)),
                    );
                }
                // The TYPE comparison against the found global lives in the
                // check layer (`check_externals`): resolving the global's
                // spec can re-enter signature inference, which cycles here.
            }
        }
    }

    fn infer_config_resources(&mut self, db: &'db dyn WorkspaceDataBase) {
        let config = match get_scope(db, self.scope).kind {
            ScopeKind::Config(c) => c,
            _ => return,
        };

        for r in config.resources(db).iter() {
            for p in r.programs(db).iter() {
                self.infer_spec(db, p.prog_type(db));
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
            match def_map.global_variables.get(&var_name.caseless(db)) {
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
                            ConfigError::AccessDeclTypeMismatch {
                                var_origin: *var,
                                spec: decl.spec,
                                expected: declared_ty,
                                actual: var_ty,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                }
                None => {
                    self.errors.push(
                        ResolveError::NoItemInScope {
                            expr: decl.variable,
                            scope: self.scope,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
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
                    check_test_visibility(db, &call_site, pou.get_scope_id(db), &mut self.errors);
                    // Reached through a USING: the import itself was refused.
                    match using {
                        Some(using) => {
                            self.usings_used.insert(using);
                        }
                        None => check_namespace_visibility(
                            db,
                            &call_site,
                            pou.get_scope_id(db),
                            &mut self.errors,
                        ),
                    }
                }
                NameResolution::Program(prog) => {
                    check_test_visibility(db, &call_site, prog.scope_id(db), &mut self.errors);
                    check_namespace_visibility(db, &call_site, prog.scope_id(db), &mut self.errors);
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

                    // Design 1: an interface as a struct field is stored state —
                    // reject it (incl. nested, e.g. a field of `ARRAY OF ITF1`).
                    if let Some(interface) = spec_interface(db, field.spec(db)) {
                        self.errors.push(
                            OopError::InterfaceNotAllowedNested {
                                interface,
                                spec: field.spec(db),
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
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
                                .to_diagnostic(db, self.scope.file(db)),
                            );
                        }
                        _ => {
                            self.errors.push(
                                ResolveError::NoNamespaceItemFound {
                                    path: target.clone(),
                                }
                                .to_diagnostic(db, self.scope.file(db)),
                            );
                        }
                    }
                }
            }
            Type::Function(_) => {
                self.errors.push(
                    TypeError::FunctionAsType {
                        expr: spec,
                        ty: typ,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            _ => (),
        };
        self.type_of_specs.insert(spec, typ);
        typ
    }
}
