
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo, Modifier,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError, e5_inheritance::InheritanceError,
    },
    hir_def::{
        pous::pou::Pou,
        scope::ScopeKind,
        semantic_index::get_scope,
    },
    hir_ty::{
        signature::{Signature, inheritance::{MethodRef, inherited_methods}}, ty::Type
    },
};

impl<'db> Signature<'db> {
    pub(crate) fn check_inheritance(&mut self, db: &'db dyn WorkspaceDataBase) {
        let implementer = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => pou,
            _ => return,
        };

        let declared_methods = &implementer.get_scope_id(db).def_map(db).declared_methods;
        let inherited_methods = inherited_methods(db, implementer);

        if let Pou::Class(cl) = implementer {
            // If the class is abstract, it must have at least one abstract method
            if cl.modifier(db).contains(Modifier::ABSTRACT)
                && !declared_methods
                    .iter()
                    .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
            {
                self.errors.push(
                    InheritanceError::AbstractClassHasNoAbstractMethods { class: implementer }
                        .to_diagnostic(db),
                );
            };
        };

        // check dups in inherited methods
        for (m1, m2) in &inherited_methods.duplicates {
            self.errors.push(
                DuplicateError::InheritedMethod {
                    method1: *m1,
                    method2: *m2,
                }
                .to_diagnostic(db),
            );
        }

        // check unresolved
        for unresolved in &inherited_methods.unresolved {
            self.errors.push(
                ResolveError::NoNamespaceItemFound {
                    path: unresolved.clone(),
                }
                .to_diagnostic(db),
            );
        }

        // look at the inherited methods first
        for (inherited_name, inherited_method) in inherited_methods.methods.iter() {
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
                            .to_diagnostic(db),
                        );
                    }
                    // Override of method without override
                    (_, Modifier::EMPTY) => {
                        self.errors.push(
                            InheritanceError::MissingOverride {
                                base_method: inherited_method,
                                derived_method: *declared_method,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    _ => {}
                }
            } else {
                // inherited method is not present

                // method is from an interface
                if inherited_method.is_prototype() {
                    self.errors.push(
                        InheritanceError::UnimplementedInterfaceMethod {
                            implementer,
                            method: inherited_method,
                        }
                        .to_diagnostic(db),
                    );
                }

                if let Modifier::ABSTRACT = inherited_method.modifier(db) {
                    self.errors.push(
                        InheritanceError::MissingAbstractMethod {
                            implementer,
                            base_method: inherited_method,
                        }
                        .to_diagnostic(db),
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
                    .to_diagnostic(db),
                );
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
                            .to_diagnostic(db),
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
                        .to_diagnostic(db),
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
            .to_diagnostic(db),
        );
    }

    for (var1, var2) in sig1.iter().zip(sig2.iter()) {
        let var1_typ = Type::new_var(db, *var1);
        let var2_typ = Type::new_var(db, *var2);

        if !var1_typ.normalize(db).eq(&var2_typ.normalize(db)) {
            errors.push(
                InheritanceError::SignatureTypeMismatch {
                    expected: var1_typ,
                    got: var2_typ,
                    method: m1,
                }
                .to_diagnostic(db),
            )
        }
    }
}
