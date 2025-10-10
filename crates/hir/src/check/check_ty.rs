use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{analysis_error::AnalysisError, ty::TyError},
    hir_ty::ty::{Ty, TyKind},
};

pub fn check_ty<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>, errors: &mut Vec<AnalysisError<'db>>) {
    match ty.kind(db) {
        TyKind::Unresolved(unresolved) => {
            errors.push(
                TyError::UnresolvedNamespace {
                    ty,
                    path: *unresolved,
                }
                .into(),
            );
        }
        TyKind::Recursive => {
            errors.push(TyError::Recursive { origin: ty }.into());
        }
        _ => {}
    }
}
