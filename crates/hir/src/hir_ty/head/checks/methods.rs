// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasModifiers, HasName, HasVisibility, HirNodeInfo, Modifier, Visibility,
    check::errors::{ToIdeDiagnostic, e01_duplicates::DuplicateError, e11_oop::OopError},
    hir_def::{
        pous::{pou::Pou, variable::VariableDecl},
        scope::ScopeKind,
        semantic_index::get_scope,
    },
    hir_ty::{
        head::init_inference::InitInference,
        infer::Infer,
        oop::{MethodRef, instance_members},
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
        let members = crate::hir_ty::oop::class_members(db, implementer);

        for base in crate::hir_ty::oop::written_bases(db, implementer) {
            let Some(target) = base.target else {
                continue;
            };
            let site = crate::CallSite::from_scoped(db, &base.spec);
            // A FUNCTION named as a base is already E0316.
            let error = if !base.fits() && !matches!(target, Pou::Function(_)) {
                OopError::WrongBaseKind {
                    pou: implementer,
                    base: target,
                    role: base.role,
                    site,
                }
            // FINAL closes a type to extension. The method-level rule was
            // enforced (E1114) while this one was not, so FINAL on a CLASS or
            // FUNCTION_BLOCK header meant nothing at all.
            } else if base.fits()
                && base.role == crate::hir_ty::oop::BaseRole::Extends
                && target.modifier(db).contains(Modifier::FINAL)
            {
                OopError::ExtendsFinalPou {
                    derived: implementer,
                    base: target,
                    extends: site,
                }
            } else {
                continue;
            };
            self.errors
                .push(error.to_diagnostic(db, self.scope.file(db)));
        }

        // IEC 6.6.7: an ABSTRACT method makes its POU incomplete, so the POU
        // must say so. Unenforced, the method had no body, nothing obliged a
        // derived POU to supply one, and calling it returned 0.
        if matches!(implementer, Pou::Class(_) | Pou::FunctionBlock(_))
            && !implementer.modifier(db).contains(Modifier::ABSTRACT)
        {
            for method in declared_methods.values() {
                if method.get_modifiers(db).contains(Modifier::ABSTRACT) {
                    self.errors.push(
                        OopError::AbstractMethodInConcretePou {
                            pou: implementer,
                            method: *method,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }

        // An ABSTRACT method declares what a derived block implements: its
        // statements would never run, and nothing could call them.
        for method in declared_methods.values() {
            if let MethodRef::Declared(decl) = method
                && method.get_modifiers(db).contains(Modifier::ABSTRACT)
                && !decl.stmts(db).is_empty()
            {
                self.errors.push(
                    OopError::AbstractMethodWithBody { method: *method }
                        .to_diagnostic(db, self.scope.file(db)),
                );
            }
        }

        for (m1, m2) in &members.duplicates {
            self.errors.push(
                DuplicateError::InheritedMethod {
                    method1: *m1,
                    method2: *m2,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }

        // Each method it declares against the one of a base it redeclares.
        for (name, base) in &members.overridden {
            let base_method = base.method;
            let Some(own) = declared_methods.get(name) else {
                continue;
            };
            check_signature(db, base_method, *own, &mut self.errors);
            // Each rule reads the flag it is about: `FINAL OVERRIDE` is both,
            // and a set compared whole missed it on either side.
            let (base_modifiers, own_modifiers) =
                (base_method.get_modifiers(db), own.get_modifiers(db));
            // A FINAL method closes its name, OVERRIDE or not.
            if base_modifiers.contains(Modifier::FINAL) {
                self.errors.push(
                    OopError::OverrideFinalMethod {
                        base_method,
                        derived_method: *own,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            // OVERRIDE is required when the base method is a concrete
            // (non-abstract) declared method. For interface prototypes and
            // abstract methods it is optional: the implementer must
            // provide a body regardless.
            if !own_modifiers.contains(Modifier::OVERRIDE)
                && !base_method.is_prototype()
                && !base_modifiers.contains(Modifier::ABSTRACT)
            {
                self.errors.push(
                    OopError::MissingOverride {
                        base_method,
                        derived_method: *own,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            // An INTERFACE's method is public, so is its implementation: a
            // call through the interface reaches it from anywhere.
            if base_method.is_prototype() && !is_public(own.get_visibility(db)) {
                self.errors.push(
                    OopError::ImplementationNotPublic {
                        method: *own,
                        interface: base.owner,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
        }

        // An INTERFACE it names may be implemented by a method it inherits.
        // That method is checked here, at the IMPLEMENTS that asks for it:
        // its own POU may implement no INTERFACE at all, and one that
        // implements this one was checked there.
        let mut checked = rustc_hash::FxHashSet::default();
        for base in crate::hir_ty::oop::written_bases(db, implementer) {
            let Some(named @ Pou::Interface(_)) = base.target else {
                continue;
            };
            let site = crate::CallSite::from_scoped(db, &base.spec);
            for (name, prototype) in &crate::hir_ty::oop::class_members(db, named).methods {
                let (Some(inherited), Pou::Interface(declaring)) =
                    (members.methods.get(name), prototype.owner)
                else {
                    continue;
                };
                if inherited.owner == implementer
                    || !inherited.method.is_declared()
                    || crate::hir_ty::oop::ancestry(db, inherited.owner).implements(declaring)
                    || !checked.insert((prototype.method, inherited.method))
                {
                    continue;
                }
                let mut found = Vec::new();
                check_signature(db, prototype.method, inherited.method, &mut found);
                if !is_public(inherited.method.get_visibility(db)) {
                    found.push(
                        OopError::ImplementationNotPublic {
                            method: inherited.method,
                            interface: prototype.owner,
                        }
                        .to_diagnostic(db, inherited.method.get_scope_id(db).file(db)),
                    );
                }
                // The method is fine in its own POU: what is wrong is this one
                // taking it as an implementation, so that is where it shows.
                for mut diagnostic in found {
                    diagnostic.diagnostic.range =
                        crate::denormalize(db, self.scope.file(db), &site.get_span(db))
                            .unwrap_or_default();
                    diagnostic.with_related(ide_diagnostic::Related::new(
                        format!(
                            "'{}' is inherited from '{}', declared here",
                            inherited.method.get_name_with_case(db).text(db),
                            inherited.owner.get_name_with_case(db).text(db),
                        ),
                        inherited.method.get_scope_id(db).file(db),
                        inherited.method.get_name_span(db),
                    ));
                    self.errors.push(diagnostic);
                }
            }
        }

        // What it inherits without declaring: a concrete POU owes a body for
        // every prototype, and for every ABSTRACT method unless it is
        // ABSTRACT itself, passing the obligation down.
        for (_, inherited) in members.inherited(implementer) {
            let method = inherited.method;
            if method.is_prototype() && !matches!(implementer, Pou::Interface(_)) {
                self.errors.push(
                    OopError::UnimplementedInterfaceMethod {
                        implementer,
                        method,
                        declared_by: inherited.owner,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            if method.get_modifiers(db).contains(Modifier::ABSTRACT)
                && !implementer.modifier(db).contains(Modifier::ABSTRACT)
            {
                self.errors.push(
                    OopError::MissingAbstractMethod {
                        implementer,
                        base_method: method,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
        }

        // OVERRIDE with nothing to override.
        for (name, own) in declared_methods {
            if own.get_modifiers(db).contains(Modifier::OVERRIDE)
                && !members.overridden.contains_key(name)
            {
                self.errors.push(
                    OopError::EmptyOverride { base_method: *own }
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
        // FB or CLASS alike. `instance_members` is the flattened view
        // (base-most first), so the nearest inherited declaration per name
        // is the LAST entry a base owns; no chain walk here.
        let own: &[VariableDecl<'db>] = match implementer {
            Pou::FunctionBlock(fb) => fb.variables(db),
            Pou::Class(cl) => cl.variables(db),
            _ => &[],
        };
        if !own.is_empty() {
            let mut inherited: FxHashMap<_, VariableDecl<'db>> = FxHashMap::default();
            for m in instance_members(db, implementer) {
                if m.owner != implementer {
                    inherited.insert(m.var.get_name_ident(db), m.var);
                }
            }
            for v in own {
                if let Some(base_decl) = inherited.get(&v.get_name_ident(db)) {
                    // Two VAR_EXTERNALs name the same global; neither owns
                    // storage, so nothing is shadowed — and redeclaring is
                    // the only way the derived body reaches the global.
                    if v.is_external(db) && base_decl.is_external(db) {
                        continue;
                    }
                    self.errors.push(
                        OopError::InheritedMemberShadowed {
                            derived: *v,
                            base: *base_decl,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }
    }

    pub(crate) fn check_methods(&mut self, db: &'db dyn WorkspaceDataBase) {
        if let Some(methods) = self.scope.method_declarations(db) {
            let mut seen = FxHashMap::default();
            for method in methods.iter() {
                match seen.get(&method.name(db)) {
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
                        seen.insert(method.name(db), *method);
                    }
                }
            }
        };

        if let Some(prototypes) = self.scope.method_prototypes(db) {
            let mut seen_prots = FxHashMap::default();
            for method in prototypes.iter() {
                match seen_prots.get(&method.name(db)) {
                    Some(prev) => self.errors.push(
                        DuplicateError::MethodProt {
                            method1: *method,
                            method2: *prev,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    ),
                    None => {
                        seen_prots.insert(method.name(db), *method);
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
    // The parameters are the signature: a method's own VAR and VAR_TEMP
    // are not, and a prototype has none.
    let parameters = |method: MethodRef<'db>| -> Vec<VariableDecl<'db>> {
        method
            .variables(db)
            .iter()
            .copied()
            .filter(|var| var.is_input(db) || var.is_output(db) || var.is_in_out(db))
            .collect()
    };
    let (sig1, sig2) = (parameters(m1), parameters(m2));
    // Each error points at `m2`, whose span is read in its own file.
    let file = m2.get_scope_id(db).file(db);
    if sig1.len() != sig2.len() {
        errors.push(
            OopError::SignatureParametersCountMismatch {
                m1,
                expected: sig1.len(),
                m2,
                got: sig2.len(),
            }
            .to_diagnostic(db, file),
        );
    }

    // The RETURN is part of the signature too: comparing only the parameter
    // list let `METHOD M : INT` be implemented as `M : REAL` — invalid wasm
    // at exit 0 through the monomorphized call, and silently wrong values
    // for a same-lane divergence like INT vs DINT.
    // By structure: two array types declared apart are two salsa values.
    let ret1 = m1.return_type(db).map(|s| s.infer(db).normalize(db));
    let ret2 = m2.return_type(db).map(|s| s.infer(db).normalize(db));
    let same_return = match (ret1, ret2) {
        (Some(ret1), Some(ret2)) => crate::hir_ty::infer::coerce::same_type(db, ret1, ret2),
        (ret1, ret2) => ret1 == ret2,
    };
    if !same_return {
        errors.push(
            OopError::SignatureReturnMismatch {
                expected: ret1,
                got: ret2,
                method: m2,
                base: m1,
            }
            .to_diagnostic(db, file),
        );
    }

    for (var1, var2) in sig1.iter().zip(sig2.iter()) {
        let var1_typ = Type::new_var(db, *var1);
        let var2_typ = Type::new_var(db, *var2);

        debug_assert!(var1.scope_id(db) == m1.get_scope_id(db));
        debug_assert!(var2.scope_id(db) == m2.get_scope_id(db));
        debug_assert!(var1.scope_id(db) != var2.scope_id(db));

        // One complaint per position: a different NAME means the type is
        // being compared against the wrong counterpart, so stop there.
        if var1.get_name_ident(db) != var2.get_name_ident(db) {
            errors.push(
                OopError::SignatureNameMismatch {
                    method: m2,
                    base_param: *var1,
                    param: *var2,
                }
                .to_diagnostic(db, file),
            );
            continue;
        }
        if var1.kind(db) != var2.kind(db) {
            errors.push(
                OopError::SignatureSectionMismatch {
                    method: m2,
                    base_param: *var1,
                    param: *var2,
                }
                .to_diagnostic(db, file),
            );
            continue;
        }
        if !crate::hir_ty::infer::coerce::same_type(
            db,
            var1_typ.normalize(db),
            var2_typ.normalize(db),
        ) {
            errors.push(
                OopError::SignatureTypeMismatch {
                    expected: var1_typ,
                    got: var2_typ,
                    method: m2,
                    base_param: *var1,
                    param: *var2,
                }
                .to_diagnostic(db, file),
            )
        } else if var1.is_input(db) && !same_default(db, *var1, *var2) {
            errors.push(
                OopError::SignatureDefaultMismatch {
                    method: m2,
                    base_param: *var1,
                    param: *var2,
                }
                .to_diagnostic(db, file),
            )
        }
    }
}

/// Whether two inputs default alike: neither has a default, or both fold
/// to the same integer, or are written the same.
fn same_default<'db>(
    db: &'db dyn WorkspaceDataBase,
    a: VariableDecl<'db>,
    b: VariableDecl<'db>,
) -> bool {
    use crate::hir_ty::{infer::const_eval::spec_value, resolver::func_call::input_default};
    match (input_default(db, a), input_default(db, b)) {
        (None, None) => true,
        (Some(a), Some(b)) => match (spec_value(db, a), spec_value(db, b)) {
            (Some(a), Some(b)) => a == b,
            _ => a.as_call_site(db).to_string(db) == b.as_call_site(db).to_string(db),
        },
        _ => false,
    }
}

/// An access specifier that hides nothing: PUBLIC, or none written.
fn is_public(visibility: Visibility) -> bool {
    visibility.is_empty() || visibility.contains(Visibility::PUBLIC)
}
