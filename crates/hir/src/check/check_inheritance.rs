use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::{
        check_semantic_index::Check,
        errors::{inheritance::MethodError, sem_errors::AnalysisError},
    },
    hir_def::{modifier::Modifier, pous::class::Class},
    hir_ty::{
        inheritance_solver::method_table,
        ty::{Ty, TyKind},
    },
};

pub fn check_methods<'db>(
    db: &'db dyn BaseDatabase,
    implementer: Ty<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) {
    let table = method_table(db, implementer);

    if let TyKind::Class { .. } = implementer.kind(db)
        && implementer.modifier(db).contains(Modifier::ABSTRACT)
        && !table
            .declared_methods
            .iter()
            .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
    {
        errors.push(AnalysisError::MethodError(
            MethodError::AbstractClassHasNoAbstractMethods { class: implementer },
        ));
    }

    // look at the inherited methods first
    for (inherited_name, inherited_method) in &table.inherited_methods {
        // method is inherited from a base interface/class
        if let Some(declared_method) = table.declared_methods.get(inherited_name) {
            match (inherited_method.modifier(db), declared_method.modifier(db)) {
                // Override of a final method
                (Modifier::FINAL, Modifier::OVERRIDE) => {
                    errors.push(AnalysisError::MethodError(
                        MethodError::OverrideFinalMethod {
                            base_method: *inherited_method,
                            derived_method: *declared_method,
                        },
                    ));
                }
                // Override of method without override
                (_, Modifier::EMPTY) => {
                    errors.push(AnalysisError::MethodError(MethodError::MissingOverride {
                        base_method: *inherited_method,
                        derived_method: *declared_method,
                    }));
                }
                _ => {}
            }
        } else {
            // inherited method is not present

            // method is from an interface
            if inherited_method.is_method_prototype(db) {
                errors.push(AnalysisError::MethodError(
                    MethodError::UnimplementedInterfaceMethod {
                        implementer,
                        method: *inherited_method,
                    },
                ));
            }

            if let Modifier::ABSTRACT = inherited_method.modifier(db) {
                errors.push(AnalysisError::MethodError(
                    MethodError::MissingAbstractMethod {
                        implementer,
                        base_method: *inherited_method,
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
