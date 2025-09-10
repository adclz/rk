use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::sem_errors::{AnalysisError, MethodError},
    hir_def::{interned::identifier::Ident, modifier::Modifier},
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
    {
        if !table
            .declared_methods
            .iter()
            .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
        {
            errors.push(AnalysisError::MethodError(
                MethodError::AbstractClassHasNoAbstractMethods { class: implementer },
            ));
        }
    }

    // look at the inherited methods first
    for (inherited_name, inherited_method) in &table.inherited_methods {
        // method is inherited from a base interface/class
        if let Some(declared_method) = table.declared_methods.get(&inherited_name) {
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

            match inherited_method.modifier(db) {
                // Abstract method without implementation
                Modifier::ABSTRACT => {
                    errors.push(AnalysisError::MethodError(
                        MethodError::MissingAbstractMethod {
                            implementer,
                            base_method: *inherited_method,
                        },
                    ));
                }
                _ => {}
            }
        }
    }
    // Look at the declared methods
    let declared = &table.declared_methods;

    for (base_name, base_method) in declared {
        if let Some(_) = table.inherited_methods.get(&base_name) {
        } else {
            match base_method.modifier(db) {
                Modifier::OVERRIDE => {
                    errors.push(AnalysisError::MethodError(MethodError::EmptyOverride {
                        base_method: *base_method,
                    }));
                }
                _ => {}
            }
        }
    }
}
