use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{sem_errors::AnalysisError, ty::TyError},
    hir_ty::{
        array_resolver::resolve_range,
        expr_resolver::{resolve_expr},
        ty::{Ty, TyKind},
    },
};

pub fn check_ty<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>, errors: &mut Vec<AnalysisError<'db>>) {
    match ty.kind(db) {
        TyKind::Unresolved(unresolved) => {
            errors.push(
                TyError::UnresolvedType {
                    ty,
                    path: unresolved.clone(),
                }
                .into(),
            );
        }
        TyKind::Target(target) => {
            // Check infinite recursion
            if let TyKind::Recursive = target.kind(db) {
                errors.push(TyError::ReferenceRecursive { origin: ty, target }.into());
            } else {
                check_ty(db, target, errors);
            }
        }
        TyKind::Recursive => {
            errors.push(TyError::Recursive { origin: ty }.into());
        }
        TyKind::Array { typ, ranges } => {
            check_ty(db, typ, errors);
            for range in ranges {
                let lower = resolve_expr(db, range.0);
                let upper = resolve_expr(db, range.1);

                match (resolve_range(db, range.0), resolve_range(db, range.1)) {
                    (Some(lower_range), Some(upper_range)) => {
                        if lower_range > upper_range {
                            errors.push(
                                TyError::InferiorUpperBound {
                                    array: ty,
                                    lower: lower_range,
                                    upper: upper_range,
                                    upper_expr: *upper,
                                }
                                .into(),
                            );
                        }
                    }
                    (None, _) => {
                        errors.push(
                            TyError::InvalidArrayLowerValue {
                                array: ty,
                                value: *lower,
                            }
                            .into(),
                        );
                    }
                    (_, None) => {
                        errors.push(
                            TyError::InvalidArrayUpperValue {
                                array: ty,
                                value: *upper,
                            }
                            .into(),
                        );
                    }
                }
            }
        }
        _ => {}
    }
}
