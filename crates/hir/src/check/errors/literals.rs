use std::fmt::Display;

use crate::{
    check::errors::{sem_errors::AnalysisError, stmt::StmtError},
    hir_ty::{expr_resolver::ResolvedExpr, ty::Ty},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LitCheckError {
    TypeMismatch(String),
    InvalidFormat { kind: &'static str, msg: String },
    OutOfRange(String),
}

impl Display for LitCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LitCheckError::OutOfRange(msg) => write!(f, "Out of range: {msg}"),
            LitCheckError::TypeMismatch(err) => write!(f, "Type mismatch: {err}"),
            LitCheckError::InvalidFormat { kind, msg } => {
                write!(f, "Invalid format for {kind}: {msg}")
            }
        }
    }
}

impl<'db> From<(LitCheckError, Ty<'db>, ResolvedExpr<'db>)> for AnalysisError<'db> {
    fn from((err, ty, expr): (LitCheckError, Ty<'db>, ResolvedExpr<'db>)) -> Self {
        AnalysisError::StmtError(StmtError::LitCheckError {
            ty,
            literal: expr,
            err,
        })
    }
}
