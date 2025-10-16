use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        coerce::coerce_ty_with_ty,
        errors::{
            analysis_error::AnalysisError, duplicates::DuplicateError, inheritance::MethodError,
        },
    },
    hir_def::{interned::identifier::Ident, modifier::Modifier, pous::pou::{Pou, PouDecl}},
    hir_ty::{
        inheritance_solver::{method_table, MethodRef},
        ty::{Ty, TyKind},
    },
};

pub fn check_methods<'db>(
    db: &'db dyn BaseDatabase,
    implementer: PouDecl<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) {
    let table = method_table(db, implementer);

    // If the class is abstract, it must have at least one abstract method
    if let Pou::Class(class) = implementer.pou(db)
        && class.modifier(db).contains(Modifier::ABSTRACT)
        && !table
            .declared_methods
            .iter()
            .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
    {
        errors.push(AnalysisError::MethodError(
            MethodError::AbstractClassHasNoAbstractMethods { class: implementer },
        ));
    }

    // check dups in declared methods
    for (m1, m2) in &table.declared_duplicates {
        errors.push(
            DuplicateError::Method {
                method1: *m1,
                method2: *m2,
            }
            .into(),
        );
    }

    // check dups in inherited methods
    for (m1, m2) in &table.inherited_duplicates {
        errors.push(
            DuplicateError::InheritedMethod {
                method1: *m1,
                method2: *m2,
            }
            .into(),
        );
    }

    // look at the inherited methods first
    for (inherited_name, inherited_method) in &table.inherited_methods {
        let inherited_method = inherited_method.method;
        // method is inherited from a base interface/class
        if let Some(declared_method) = table.declared_methods.get(inherited_name) {
            check_signature(db, inherited_method, *declared_method, errors);

            match (inherited_method.modifier(db), declared_method.modifier(db)) {
                // Override of a final method
                (Modifier::FINAL, Modifier::OVERRIDE) => {
                    errors.push(AnalysisError::MethodError(
                        MethodError::OverrideFinalMethod {
                            base_method: inherited_method,
                            derived_method: *declared_method,
                        },
                    ));
                }
                // Override of method without override
                (_, Modifier::EMPTY) => {
                    errors.push(AnalysisError::MethodError(MethodError::MissingOverride {
                        base_method: inherited_method,
                        derived_method: *declared_method,
                    }));
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
                ));
            }

            if let Modifier::ABSTRACT = inherited_method.modifier(db) {
                errors.push(AnalysisError::MethodError(
                    MethodError::MissingAbstractMethod {
                        implementer,
                        base_method: inherited_method,
                    },
                ));
            }
        }
    }
    // Look at the declared methods
    let declared = &table.declared_methods;

    for (base_name, base_method) in declared {
        if table.inherited_methods.contains_key(base_name) {
        } else if base_method.modifier(db) == Modifier::OVERRIDE {
            errors.push(AnalysisError::MethodError(MethodError::EmptyOverride {
                base_method: *base_method,
            }));
        }
    }
}

fn check_signature<'db>(
    db: &'db dyn BaseDatabase,
    m1: MethodRef<'db>,
    m2: MethodRef<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
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
            .into(),
        );
    }

    sig1.iter().zip(sig2.iter()).for_each(|(var1, var2)| {
        if let Err(err) = coerce_ty_with_ty(db, var1.spec(db).spec_to_ty(db), var2.spec(db).spec_to_ty(db)) {
            errors.push(MethodError::SignatureParametersTypeMismatch { param: *var2, err }.into());
        }
    });
}
