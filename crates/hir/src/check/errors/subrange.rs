use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    check::errors::{
        analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
        coerce::{ExprMismatch, TypeMismatch},
    }, hir_def::{expressions::spec::{Enum, Spec}, interned::{identifier::SpanIdent, namespace::SpanNamespaceAccess}}, hir_ty::{expr_resolver::ResolvedExpr, ty::Ty}, HirNodeInfo, TypeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SubRangeError<'db> {
    // Subrange
    InvalidSubrangeType {
        typ: Spec<'db>,
    },
    InvalidSubrangeStart {
        expr: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
    InvalidSubrangeEnd {
        expr: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
}

impl<'db> From<SubRangeError<'db>> for AnalysisError<'db> {
    fn from(err: SubRangeError<'db>) -> Self {
        AnalysisError::SubRangeError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for SubRangeError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            SubRangeError::InvalidSubrangeType { typ } => {
                let mut diag = diag()
                    .message(format!("invalid subrange type '{}'", typ.to_ty(db).type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(typ.get_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for SUBRANGE".to_string());

                diag
            }
            SubRangeError::InvalidSubrangeStart { expr, err } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid start value for subrange: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db))
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);

                diag
            }
            SubRangeError::InvalidSubrangeEnd { expr, err } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid end value for subrange: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db))
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);

                diag
            }
        }
    }
}
