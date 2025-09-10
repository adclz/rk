use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{diag, IdeDiagnostic};

use crate::{check::errors::sem_errors::{AnalysisError, ToIdeDiagnostic}, hir_def::{expressions::expression::PathExpr, scope::FileScopeId}, hir_ty::ty::Ty, to_proto::ToProto};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum PathExprError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: FileScopeId<'db>,
    },
    UnknownField {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    UnexpectedIndex {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    NotAnArray {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    NotAReference {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
}


impl<'db> From<PathExprError<'db>> for AnalysisError<'db> {
    fn from(err: PathExprError<'db>) -> Self {
        AnalysisError::PathExprError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for PathExprError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::NoItemInScope { expr, scope } => diag()
                .message(format!(
                    "no item '{}' in scope",
                    expr.to_string(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::UnknownField { ty: origin, expr } => diag()
                .message(format!(
                    "field {} not found in type",
                    expr.to_string(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::UnexpectedIndex { ty: origin, expr } => diag()
                .message("unexpected index expression".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::NotAReference { ty: origin, expr } => diag()
                .message("type can not be dereferenced".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::NotAnArray { ty: origin, expr } => diag()
                .message("type is not an array".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
        }
    }
}
