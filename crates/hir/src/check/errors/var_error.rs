use auto_lsp::default::db::BaseDatabase;

use crate::{check::errors::{analysis_error::DiagnosticDescription, path_error::PathResolveError}, hir_ty::ty_var_access_resolver::CallSite};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VarResolveError<'db> {
    Unknown { call_site: CallSite<'db> },
    PathResolveError { err: PathResolveError<'db> },
}

impl<'db> DiagnosticDescription<'db> for VarResolveError<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            VarResolveError::PathResolveError { err } => err.description(db),
            VarResolveError::Unknown { call_site } => {
                format!("no item '{}' in scope", call_site.to_string(db))
            }
        }
    }

    fn note(&self, db: &'db dyn BaseDatabase, diag: &mut ide_diagnostic::IdeDiagnostic) {
        match self {
            VarResolveError::PathResolveError { err } => err.note(db, diag),
            _ => {}
        }
    }

    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut ide_diagnostic::IdeDiagnostic) {
        match self {
            VarResolveError::PathResolveError { err } => err.related(db, diag),
            _ => {}
        }
    }
}
