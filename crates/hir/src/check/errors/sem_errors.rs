use core::panic;
use std::{collections::HashMap, error::Error, fmt::Display};

use auto_lsp::{
    core::{errors::PositionError, span::Span},
    default::db::{BaseDatabase, file::File},
    lsp_types::{DiagnosticSeverity, DiagnosticTag, WorkspaceEdit},
    tree_sitter::{self, Range}, 
};
use ide_diagnostic::{IdeDiagnostic, Related, action, diag, edit};

use crate::{
    check::errors::{duplicates::DuplicateError, inheritance::MethodError, path_expr::PathExprError, scope::NamespaceError, stmt::StmtError, syntax::SyntaxError}, hir_def::{
        expressions::{expression::PathExpr, statement::Stmt},
        interned::{identifier::Ident, namespace::NamespacePath},
        namespace::NamespaceDecl,
        pous::{pou::PouDecl, variable::VariableDecl},
        scope::FileScopeId,
        using::Using,
    }, hir_ty::{
        expr_resolver::ResolvedExpr, ty::Ty, ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::ResolvedVarResult,
    }, to_proto::ToProto
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
    MethodError(MethodError<'db>),
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
            Self::MethodError(err) => err.to_diagnostic(db),
        }
    }
}
