use core::panic;
use std::{error::Error, fmt::Display};

use auto_lsp::{core::errors::PositionError, default::db::BaseDatabase};
use ide_diagnostic::IdeDiagnostic;

use crate::check::errors::{
    array::ArrayError, duplicates::DuplicateError, enum_::EnumError, inheritance::MethodError,
    init_expr::InitExprError, path_error::AccessError, scope::NamespaceError, stmt::StmtError,
    subrange::SubRangeError, syntax::SyntaxError, visibility::VisibilityError,
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
    StmtError(StmtError<'db>),
    InitExprError(InitExprError<'db>),
    MethodError(MethodError<'db>),
    ArrayError(ArrayError<'db>),
    SubRangeError(SubRangeError<'db>),
    EnumError(EnumError<'db>),
    VisibilityError(VisibilityError<'db>),
    AccessError(AccessError<'db>),
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
            Self::StmtError(err) => err.to_diagnostic(db),
            Self::InitExprError(err) => err.to_diagnostic(db),
            Self::MethodError(err) => err.to_diagnostic(db),
            Self::ArrayError(err) => err.to_diagnostic(db),
            Self::SubRangeError(err) => err.to_diagnostic(db),
            Self::EnumError(err) => err.to_diagnostic(db),
            Self::VisibilityError(err) => err.to_diagnostic(db),
            Self::AccessError(err) => err.to_diagnostic(db),
        }
    }
}

pub trait DiagnosticDescription<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String;
    fn note(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {}
    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {}
}
