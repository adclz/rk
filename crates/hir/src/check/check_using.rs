use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::check_semantic_index::Check, hir_def::using::Using, hir_ty::name_res::namespace_index,
};

impl<'db> Check<'db> for Using<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        if namespace_index(db, *self.path(db)).is_empty() {
            //.push(AccessError::InvalidUsingDirective { using: *self }.to_diagnostic(db))
        }
    }
}
