use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{check::{check_semantic_index::Check, errors::{analysis_error::ToIdeDiagnostic, path_error::AccessError}}, hir_def::using::Using, hir_ty::name_res::global_namespace_index};

impl<'db> Check<'db> for Using<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        if let None =  global_namespace_index(db)
            .get(&self.path(db)) {
                errors.push(AccessError::InvalidUsingDirective { using: *self }.to_diagnostic(db))
            }
    }
}