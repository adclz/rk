use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName, HirNodeInfo, Modifier,
    check::{
        check_semantic_index::Check,
        errors::{
            analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError,
            e2_resolve::ResolveError, e5_inheritance::InheritanceError,
        },
    },
    hir_def::pous::{class::MethodDecl, interface::MethodPrototype, pou::Pou},
    hir_ty::{
        body_inference::infer_body_scope,
        inheritance_solver::{MethodRef, inherited_methods},
        ty::Type,
    },
};

impl<'db> Check<'db> for Vec<MethodDecl<'db>> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        let mut seen = FxHashMap::default();
        for method in self {
            for error in &infer_body_scope(db, method.get_scope_id(db)).errors {
                errors.push(error.clone());
            }

            if let Some(prev) = seen.get(&method.get_name_ident(db)) {
                errors.push(
                    DuplicateError::MethodDecl {
                        method1: *prev,
                        method2: *method,
                    }
                    .to_diagnostic(db),
                );
            } else {
                seen.insert(method.get_name_ident(db), *method);
            }
        }
    }
}

impl<'db> Check<'db> for Vec<MethodPrototype<'db>> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        let mut seen = FxHashMap::default();
        for method in self {
            if let Some(prev) = seen.get(&method.get_name_ident(db)) {
                errors.push(
                    DuplicateError::MethodProt {
                        method1: *prev,
                        method2: *method,
                    }
                    .to_diagnostic(db),
                );
            } else {
                seen.insert(method.name(db), *method);
            }
        }
    }
}

pub fn check_inheritance<'db>(
    db: &'db dyn WorkspaceDataBase,
    implementer: Pou<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let declared_methods = &implementer.get_scope_id(db).def_map(db).declared_methods;
    let inherited_methods = inherited_methods(db, implementer);

    if let Pou::Class(cl) = implementer {
        // If the class is abstract, it must have at least one abstract method
        if cl.modifier(db).contains(Modifier::ABSTRACT)
            && !declared_methods
                .iter()
                .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
        {
            errors.push(
                InheritanceError::AbstractClassHasNoAbstractMethods { class: implementer }
                    .to_diagnostic(db),
            );
        };
    };

    // check dups in inherited methods
    for (m1, m2) in &inherited_methods.duplicates {
        errors.push(
            DuplicateError::InheritedMethod {
                method1: *m1,
                method2: *m2,
            }
            .to_diagnostic(db),
        );
    }

    // check unresolved
    for unresolved in &inherited_methods.unresolved {
        errors.push(
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
            check_signature(db, inherited_method, *declared_method, errors);

            match (inherited_method.modifier(db), declared_method.modifier(db)) {
                // Override of a final method
                (Modifier::FINAL, Modifier::OVERRIDE) => {
                    errors.push(
                        InheritanceError::OverrideFinalMethod {
                            base_method: inherited_method,
                            derived_method: *declared_method,
                        }
                        .to_diagnostic(db),
                    );
                }
                // Override of method without override
                (_, Modifier::EMPTY) => {
                    errors.push(
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
                errors.push(
                    InheritanceError::UnimplementedInterfaceMethod {
                        implementer,
                        method: inherited_method,
                    }
                    .to_diagnostic(db),
                );
            }

            if let Modifier::ABSTRACT = inherited_method.modifier(db) {
                errors.push(
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
            errors.push(
                InheritanceError::EmptyOverride {
                    base_method: *base_method,
                }
                .to_diagnostic(db),
            );
        }
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
