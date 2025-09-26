use auto_lsp::default::db::BaseDatabase;

use crate::check::errors::{analysis_error::DiagnosticDescription, path_error::PathResolveError};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VarResolveError<'db> {
    NotFound,
    InvalidType,
    PathResolveError { err: PathResolveError<'db> },
}

impl<'db> DiagnosticDescription<'db> for VarResolveError<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            VarResolveError::PathResolveError { err } => err.description(db),
            VarResolveError::InvalidType => {
                format!("type not found")
            }
            VarResolveError::NotFound => {
                format!("variable not found")
            }
        }
    }

    fn note(&self, db: &'db dyn BaseDatabase, diag: &mut ide_diagnostic::IdeDiagnostic) {
        match self {
            VarResolveError::PathResolveError { err } => err.note(db, diag),
            _ => { /* no note */ }
        }
    }

    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut ide_diagnostic::IdeDiagnostic) {
        match self {
            VarResolveError::PathResolveError { err } => err.related(db, diag),
            _ => { /* no related info */ }
        }
    }
}
