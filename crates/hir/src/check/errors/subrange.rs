use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::expressions::spec::Spec,
    hir_ty::ty::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SubRangeError<'db> {
    // Subrange
    InvalidSubrangeType { spec: Spec<'db>, typ: Type<'db> },
    /*InvalidSubrangeStart {
        expr: Expr<'db>,
        err: ExprMismatch<'db>,
    },
    InvalidSubrangeEnd {
        expr: Expr<'db>,
        err: ExprMismatch<'db>,
    },*/
}

impl<'db> From<SubRangeError<'db>> for AnalysisError<'db> {
    fn from(err: SubRangeError<'db>) -> Self {
        AnalysisError::SubRangeError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for SubRangeError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            SubRangeError::InvalidSubrangeType { spec, typ } => {
                let mut diag = diag()
                    .message(format!("invalid subrange type '{}'", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(spec.get_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for SUBRANGE".to_string());

                diag
            } /*SubRangeError::InvalidSubrangeStart { expr, err } => {
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
              }*/
        }
    }
}
