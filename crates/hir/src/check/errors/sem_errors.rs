use core::panic;
use std::{error::Error, fmt::Display};

use auto_lsp::{core::errors::PositionError, default::db::BaseDatabase};
use ide_diagnostic::IdeDiagnostic;

use crate::check::errors::{
    duplicates::DuplicateError, inheritance::MethodError, init_expr::InitExprError, literals::LiteralError, path_expr::PathExprError, scope::NamespaceError, stmt::StmtError, syntax::SyntaxError, ty::TyError
};

pub trait ToIdeDiagnostic<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic;
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum AnalysisError<'db> {
    // Specific
    AutoLspError(PositionError),
    SyntaxError(SyntaxError),
    NamespaceError(NamespaceError<'db>),
    DuplicateError(DuplicateError<'db>),
    PathExprError(PathExprError<'db>),
    StmtError(StmtError<'db>),
    InitExprError(InitExprError<'db>),
    MethodError(MethodError<'db>),
    TyError(TyError<'db>),
    LiteralError(LiteralError<'db>),
}

impl Error for AnalysisError<'_> {}

impl Display for AnalysisError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl From<PositionError> for AnalysisError<'_> {
    fn from(err: PositionError) -> Self {
        AnalysisError::AutoLspError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for AnalysisError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::AutoLspError(err) => panic!("A position error happened: {}", err),
            Self::SyntaxError(err) => err.to_diagnostic(db),
            Self::NamespaceError(err) => err.to_diagnostic(db),
            Self::DuplicateError(err) => err.to_diagnostic(db),
            Self::PathExprError(err) => err.to_diagnostic(db),
            Self::StmtError(err) => err.to_diagnostic(db),
            Self::InitExprError(err) => err.to_diagnostic(db),
            Self::MethodError(err) => err.to_diagnostic(db),
            Self::TyError(err) => err.to_diagnostic(db),
            Self::LiteralError(err) => err.to_diagnostic(db),
        }
    }
}
