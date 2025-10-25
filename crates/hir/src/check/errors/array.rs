use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic}, hir_def::expressions::expression::Expr, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ArrayError<'db> {
    // Arrays
    InvalidArrayLowerValue {
        value: Expr<'db>,
    },
    InvalidArrayUpperValue {
        value: Expr<'db>,
    },
    InferiorUpperBound {
        lower: u64,
        upper: u64,
        upper_expr: Expr<'db>,
    },
}

impl<'db> From<ArrayError<'db>> for AnalysisError<'db> {
    fn from(err: ArrayError<'db>) -> Self {
        AnalysisError::ArrayError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for ArrayError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            ArrayError::InvalidArrayLowerValue { value } => diag()
                .message("Invalid lower bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(value.get_span(db))
                .call(),
            ArrayError::InvalidArrayUpperValue { value } => diag()
                .message("Invalid upper bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(value.get_span(db))
                .call(),
            ArrayError::InferiorUpperBound {
                lower,
                upper,
                upper_expr,
            } => diag()
                .message("Upper bound value must be greater than lower bound value".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(upper_expr.get_span(db))
                .call(),
        }
    }
}
