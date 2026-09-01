use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName, HirNodeInfo, Modifier,
    check::errors::{
        ToIdeDiagnostic, e1_duplicates::DuplicateError, e5_inheritance::InheritanceError,
    },
    hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope},
    hir_ty::{
        head::{
            inheritance::{MethodRef, inherited_methods},
            init_inference::InitInference,
        },
        infer::Infer,
        ty::Type,
    },
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_inheritance(&mut self, db: &'db dyn WorkspaceDataBase) {
        let implementer = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => pou,
            _ => return,
        };

        let declared_methods = &implementer.get_scope_id(db).def_map(db).declared_methods;
        let inherited_methods = inherited_methods(db, implementer);

        // FINAL closes a type to extension. The method-level rule was
        // enforced (E0504) while this one was not, so FINAL on a CLASS or
        // FUNCTION_BLOCK header meant nothing at all.
        if let Some(extends) = match implementer {
            Pou::FunctionBlock(fb) => fb.extends(db),
            Pou::Class(cl) => cl.extends(db),
            _ => None,
        } && let Some(base) = crate::hir_ty::head::inheritance::base_pou(db, implementer)
            && base.modifier(db).contains(Modifier::FINAL)
        {
            self.errors.push(
                InheritanceError::ExtendsFinalPou {
                    derived: implementer,
                    base,
                    extends: crate::CallSite::from_scoped(db, extends),
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }

        // IEC 6.6.7: an ABSTRACT method makes its POU incomplete, so the POU
        // must say so. Unenforced, the method had no body, nothing obliged a
        // derived POU to supply one, and calling it returned 0.
        if matches!(implementer, Pou::Class(_) | Pou::FunctionBlock(_))
            && !implementer.modifier(db).contains(Modifier::ABSTRACT)
        {
            for method in declared_methods.values() {
                if method.modifier(db).contains(Modifier::ABSTRACT) {
                    self.errors.push(
                        InheritanceError::AbstractMethodInConcretePou {
                            pou: implementer,
                            method: *method,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }

        // check dups in inherited methods
        for (m1, m2) in &inherited_methods.duplicates {
            self.errors.push(
                DuplicateError::InheritedMethod {
                    method1: *m1,
                    method2: *m2,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }

        // look at the inherited methods first
        for (inherited_name, inherited_method) in inherited_methods.methods.iter() {
            let declared_by = inherited_method.source;
            let inherited_method = inherited_method.method;
            // method is inherited from a base interface/class
            if let Some(declared_method) = declared_methods.get(inherited_name) {
                check_signature(db, inherited_method, *declared_method, &mut self.errors);

                match (inherited_method.modifier(db), declared_method.modifier(db)) {
                    // Override of a final method
                    (Modifier::FINAL, Modifier::OVERRIDE) => {
                        self.errors.push(
                            InheritanceError::OverrideFinalMethod {
                                base_method: inherited_method,
                                derived_method: *declared_method,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                    // Override of a concrete method without OVERRIDE keyword.
                    // OVERRIDE is only required when the base method is a concrete
                    // (non-abstract) declared method. For interface prototypes and
                    // abstract methods, OVERRIDE is optional - the implementer
                    // must provide a body regardless.
                    (_, Modifier::EMPTY)
                        if !inherited_method.is_prototype()
                            && inherited_method.modifier(db) != Modifier::ABSTRACT =>
                    {
                        self.errors.push(
                            InheritanceError::MissingOverride {
                                base_method: inherited_method,
                                derived_method: *declared_method,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                    _ => {}
                }
            } else {
                // inherited method is not present

                // method is from an interface — only require implementation
                // for concrete POUs (classes, function blocks), not interfaces
                if inherited_method.is_prototype() && !matches!(implementer, Pou::Interface(_)) {
                    self.errors.push(
                        InheritanceError::UnimplementedInterfaceMethod {
                            implementer,
                            method: inherited_method,
                            declared_by,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }

                // An ABSTRACT POU is allowed to leave inherited ABSTRACT
                // methods unimplemented — passing the obligation down is what
                // an abstract intermediate class is FOR. Only a concrete POU
                // owes an implementation.
                if let Modifier::ABSTRACT = inherited_method.modifier(db)
                    && !implementer.modifier(db).contains(Modifier::ABSTRACT)
                {
                    self.errors.push(
                        InheritanceError::MissingAbstractMethod {
                            implementer,
                            base_method: inherited_method,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }
        // Look at the declared methods

        for (base_name, base_method) in declared_methods {
            if inherited_methods.methods.contains_key(base_name) {
            } else if base_method.modifier(db) == Modifier::OVERRIDE {
                self.errors.push(
                    InheritanceError::EmptyOverride {
                        base_method: *base_method,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
        }

        // Rule 3 (IEC 6.6.7.2.9): the names of the variables in the base and the
        // derived function blocks shall be unique. Walk the EXTENDS chain and
        // report any own variable whose name collides with an inherited one
        // (the derived body and the base body — reachable via SUPER() — would
        // otherwise operate on two distinct, same-named slots).
        // `def_map.local_variables` is params-only; Rule 3 covers ALL variables
        // (esp. `VAR` members), so read them from the FB directly.
        // FB or CLASS alike — the check was FB-gated at BOTH ends, so two
        // CLASSes declaring the same member shared one slot silently (and
        // with different types, emitted invalid wasm at exit 0).
        let member_vars = |pou: Pou<'db>| match pou {
            Pou::FunctionBlock(fb) => Some(fb.variables(db)),
            Pou::Class(cl) => Some(cl.variables(db)),
            _ => None,
        };
        if let Some(own) = member_vars(implementer) {
            if !own.is_empty() {
                // Nearest inherited declaration per name, walking up the chain.
                let mut inherited = FxHashMap::default();
                let mut visited = rustc_hash::FxHashSet::default();
                let mut current = extends_pou(db, implementer);
                while let Some(base) = current {
                    if !visited.insert(base) {
                        break; // guard against EXTENDS cycles (reported elsewhere)
                    }
                    if let Some(base_vars) = member_vars(base) {
                        for v in base_vars {
                            inherited
                                .entry(v.get_name_ident(db).caseless(db))
                                .or_insert(*v);
                        }
                    }
                    current = extends_pou(db, base);
                }
                for v in own {
                    if let Some(base_decl) = inherited.get(&v.get_name_ident(db).caseless(db)) {
                        // Two VAR_EXTERNALs name the same global; neither owns
                        // storage, so nothing is shadowed — and redeclaring is
                        // the only way the derived body reaches the global.
                        if v.is_external(db) && base_decl.is_external(db) {
                            continue;
                        }
                        self.errors.push(
                            InheritanceError::InheritedMemberShadowed {
                                derived: *v,
                                base: *base_decl,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                }
            }
        }
    }

    pub(crate) fn check_methods(&mut self, db: &'db dyn WorkspaceDataBase) {
        if let Some(methods) = self.scope.method_declarations(db) {
            let mut seen = FxHashMap::default();
            for method in methods.iter() {
                match seen.get(&method.name(db).caseless(db)) {
                    Some(prev) => {
                        self.errors.push(
                            DuplicateError::MethodDecl {
                                method1: *method,
                                method2: *prev,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                    None => {
                        seen.insert(method.name(db).caseless(db), *method);
                    }
                }
            }
        };

        if let Some(prototypes) = self.scope.method_prototypes(db) {
            let mut seen_prots = FxHashMap::default();
            for method in prototypes.iter() {
                match seen_prots.get(&method.name(db).caseless(db)) {
                    Some(prev) => self.errors.push(
                        DuplicateError::MethodProt {
                            method1: *method,
                            method2: *prev,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    ),
                    None => {
                        seen_prots.insert(method.name(db).caseless(db), *method);
                    }
                }
            }
        };

        self.check_inheritance(db);
    }
}

fn check_signature<'db>(
    db: &'db dyn WorkspaceDataBase,
    m1: MethodRef<'db>,
    m2: MethodRef<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let sig1 = m1.variables(db);
    let sig2 = m2.variables(db);
    if sig1.len() != sig2.len() {
        errors.push(
            InheritanceError::SignatureParametersCountMismatch {
                m1,
                expected: sig1.len(),
                m2,
                got: sig2.len(),
            }
            .to_diagnostic(db, m1.get_scope_id(db).file(db)),
        );
    }

    // The RETURN is part of the signature too: comparing only the parameter
    // list let `METHOD M : INT` be implemented as `M : REAL` — invalid wasm
    // at exit 0 through the monomorphized call, and silently wrong values
    // for a same-lane divergence like INT vs DINT.
    let ret1 = m1.return_type(db).map(|s| s.infer(db).normalize(db));
    let ret2 = m2.return_type(db).map(|s| s.infer(db).normalize(db));
    if ret1 != ret2 {
        errors.push(
            InheritanceError::SignatureReturnMismatch {
                expected: ret1,
                got: ret2,
                method: m2,
                base: m1,
            }
            .to_diagnostic(db, m1.get_scope_id(db).file(db)),
        );
    }

    for (var1, var2) in sig1.iter().zip(sig2.iter()) {
        let var1_typ = Type::new_var(db, *var1);
        let var2_typ = Type::new_var(db, *var2);

        debug_assert!(var1.scope_id(db) == m1.get_scope_id(db));
        debug_assert!(var2.scope_id(db) == m2.get_scope_id(db));
        debug_assert!(var1.scope_id(db) != var2.scope_id(db));

        if !var1_typ.normalize(db).eq(&var2_typ.normalize(db)) {
            errors.push(
                InheritanceError::SignatureTypeMismatch {
                    expected: var1_typ,
                    got: var2_typ,
                    method: m2,
                    base_param: *var1,
                    param: *var2,
                }
                .to_diagnostic(db, m1.get_scope_id(db).file(db)),
            )
        }
    }
}

/// The immediate base POU an FB or Class extends, if any (single inheritance).
fn extends_pou<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Option<Pou<'db>> {
    let spec = match pou {
        Pou::FunctionBlock(fb) => fb.extends(db)?,
        Pou::Class(cl) => cl.extends(db)?,
        _ => return None,
    };
    spec.infer(db).normalize(db).as_pou(db)
}
