use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap};

use crate::{
    check::{
        coerce::coerce_ty_with_ty,
        errors::{
            analysis_error::{AnalysisError, ToIdeDiagnostic}, duplicates::DuplicateError, inheritance::MethodError,
        },
    },
    hir_def::{modifier::Modifier, pous::pou::{Pou, PouDecl}
    },
    hir_ty::inheritance_solver::{declared_methods, inherited_methods, to_method_ref, MethodRef}, HirNodeInfo,
};

pub fn check_inheritance<'db>(
    db: &'db dyn BaseDatabase,
    implementer: PouDecl<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let declared_methods = declared_methods(db, implementer);
    let inherited_methods = inherited_methods(db, implementer);

    match implementer.pou(db) {
        Pou::Class(cl) => {
            // If the class is abstract, it must have at least one abstract method
            if cl.modifier(db).contains(Modifier::ABSTRACT)
                && !declared_methods
                    .iter()
                    .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
            {
                errors.push(AnalysisError::MethodError(
                    MethodError::AbstractClassHasNoAbstractMethods { class: implementer },
                ).to_diagnostic(db));
            };
        },
        _ => {}
    };

    check_declared_duplicates(db, to_method_ref(db, implementer), errors);

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
        errors.push(AnalysisError::MethodError(
            MethodError::UnresolvedPou { access: *unresolved }
        ).to_diagnostic(db));
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
                    errors.push(AnalysisError::MethodError(
                        MethodError::OverrideFinalMethod {
                            base_method: inherited_method,
                            derived_method: *declared_method,
                        },
                    ).to_diagnostic(db));
                }
                // Override of method without override
                (_, Modifier::EMPTY) => {
                    errors.push(AnalysisError::MethodError(MethodError::MissingOverride {
                        base_method: inherited_method,
                        derived_method: *declared_method,
                    }).to_diagnostic(db));
                }
                _ => {}
            }
        } else {
            // inherited method is not present

            // method is from an interface
            if inherited_method.is_prototype() {
                errors.push(AnalysisError::MethodError(
                    MethodError::UnimplementedInterfaceMethod {
                        implementer,
                        method: inherited_method,
                    },
                ).to_diagnostic(db));
            }

            if let Modifier::ABSTRACT = inherited_method.modifier(db) {
                errors.push(AnalysisError::MethodError(
                    MethodError::MissingAbstractMethod {
                        implementer,
                        base_method: inherited_method,
                    },
                ).to_diagnostic(db));
            }
        }
    }
    // Look at the declared methods

    for (base_name, base_method) in declared_methods {
        if inherited_methods.methods.contains_key(&base_name) {
        } else if base_method.modifier(db) == Modifier::OVERRIDE {
            errors.push(AnalysisError::MethodError(MethodError::EmptyOverride {
                base_method: *base_method,
            }).to_diagnostic(db));
        }
    }
}

fn check_declared_duplicates<'db>(
    db: &'db dyn BaseDatabase,
    methods: &[MethodRef<'db>],
    errors: &mut Vec<IdeDiagnostic>,
) {
    let mut seen = FxHashMap::default();
    for method in methods {
        if let Some(prev) = seen.get(method.name(db)) {
            errors.push(
                DuplicateError::Method {
                    method1: *prev,
                    method2: *method,
                }
                .to_diagnostic(db),
            );
        } else {
            seen.insert(*method.name(db), *method);
        }
    }
}

fn check_signature<'db>(
    db: &'db dyn BaseDatabase,
    m1: MethodRef<'db>,
    m2: MethodRef<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let sig1 = m1.variables(db);
    let sig2 = m2.variables(db);
    if sig1.len() != sig2.len() {
        errors.push(
            MethodError::SignatureParametersCountMismatch {
                m1,
                expected: sig1.len(),
                m2,
                got: sig2.len(),
            }
            .to_diagnostic(db),
        );
    }

    for (var1, var2) in sig1.iter().zip(sig2.iter()) {
        if let Err(err) = coerce_ty_with_ty(db, var1.spec(db).to_ty(db), var2.spec(db).to_ty(db)) {
            errors.push(MethodError::SignatureParametersTypeMismatch { param: *var2, err }.to_diagnostic(db));
        }
    }
}
