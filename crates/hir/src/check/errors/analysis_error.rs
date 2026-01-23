use core::panic;
use std::{error::Error, fmt::Display};

use auto_lsp::core::errors::PositionError;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::check::errors::{
    e0_syntax::SyntaxError, e1_duplicates::DuplicateError, e2_resolve::ResolveError,
    e3_type::TypeError, e4_visibility::VisibilityError, e5_inheritance::InheritanceError,
    e6_array::ArrayError, e7_enum::EnumError, e8_subrange::SubRangeError,
    e9_recursion::RecursionError, e10_control_flow::ControlFlowError,
};

pub trait ToIdeDiagnostic<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic;
}

/*
E00xx = Syntax errors
E01xx = Duplicate definitions
E02xx = Scope/resolution
E03xx = Type system
E04xx = Visibility/access
E05xx = Inheritance/methods (oop)
E06xx = Arrays
E07xx = Enums
E08xx = Subranges
E09xx = Recursion
E10xx = Control flow
E11xx = Hardware errors (direct variables, IO issues)
*/

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum AnalysisError<'db> {
    // Specific
    AutoLspError(PositionError),
    Syntax(SyntaxError),                // E0xx
    Duplicate(DuplicateError<'db>),     // E1xx
    Resolve(ResolveError<'db>),         // E2xx
    Type(TypeError<'db>),               // E3xx
    Visibility(VisibilityError<'db>),   // E4xx
    Inheritance(InheritanceError<'db>), // E5xx
    Array(ArrayError<'db>),             // E6xx
    Enum(EnumError<'db>),               // E7xx
    SubRange(SubRangeError<'db>),       // E8xx
    Recursion(RecursionError<'db>),     // E9xx
    ControlFlow(ControlFlowError<'db>), // E10xx
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
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::AutoLspError(err) => panic!("A position error happened: {}", err),
            Self::Syntax(err) => err.to_diagnostic(db),
            Self::Duplicate(err) => err.to_diagnostic(db),
            Self::Resolve(err) => err.to_diagnostic(db),
            Self::Type(err) => err.to_diagnostic(db),
            Self::Visibility(err) => err.to_diagnostic(db),
            Self::Inheritance(err) => err.to_diagnostic(db),
            Self::Array(err) => err.to_diagnostic(db),
            Self::Enum(err) => err.to_diagnostic(db),
            Self::SubRange(err) => err.to_diagnostic(db),
            Self::Recursion(err) => err.to_diagnostic(db),
            Self::ControlFlow(err) => err.to_diagnostic(db),
        }
    }
}
