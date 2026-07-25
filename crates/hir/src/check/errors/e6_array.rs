use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::expression::{Expr, InitExpr},
        interned::identifier::SpanIdent,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ArrayError<'db> {
    // Array init
    InvalidArrayLowerValue {
        value: Expr<'db>,
    },
    InvalidArrayUpperValue {
        value: Expr<'db>,
    },
    InferiorUpperBound {
        lower: i64,
        upper: i64,
        upper_expr: Expr<'db>,
    },
    TooManyElements {
        expr: InitExpr<'db>,
        dimension: usize,
        max_size: usize,
    },
    // Array access
    InvalidIndex {
        size: SpanIdent<'db>,
        err: String,
    },
    IndexOutOfBounds {
        size: SpanIdent<'db>,
        dimension: usize,
        index: u64,
        min: u64,
        max: u64,
    },
}

impl<'db> ErrorCode for ArrayError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidArrayLowerValue { .. } => "E0601",
            Self::InvalidArrayUpperValue { .. } => "E0602",
            Self::InferiorUpperBound { .. } => "E0603",
            Self::TooManyElements { .. } => "E0605",
            Self::InvalidIndex { .. } => "E0607",
            Self::IndexOutOfBounds { .. } => "E0608",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InvalidArrayLowerValue { .. }
            | Self::InvalidArrayUpperValue { .. }
            | Self::InferiorUpperBound { .. } => "invalid array bounds",
            Self::TooManyElements { .. }
            | Self::InvalidIndex { .. }
            | Self::IndexOutOfBounds { .. } => "invalid array access",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ArrayError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            ArrayError::InvalidArrayLowerValue { value } => diag()
                .message("invalid lower bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                .call(),
            ArrayError::InvalidArrayUpperValue { value } => diag()
                .message("invalid upper bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                .call(),
            ArrayError::InferiorUpperBound {
                lower,
                upper,
                upper_expr,
            } => diag()
                .message("upper bound value must be greater than lower bound value".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &upper_expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::TooManyElements {
                expr,
                dimension,
                max_size,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "too many elements in array initializer (expected at most {})",
                        max_size
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                if *dimension > 0_usize {
                    diag.with_note(format!(
                        "this error occurred in array dimension {}",
                        dimension + 1
                    ))
                }

                diag
            }
            Self::InvalidIndex { size, err } => diag()
                .message(format!("invalid index value '{}': {err}", size.text(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &size.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexOutOfBounds {
                size,
                dimension,
                index,
                min,
                max,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "index '{}' is out of bounds (expected between {} and {})",
                        index, min, max
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &size.get_span(db)).unwrap_or_default())
                    .call();

                if *dimension > 0_usize {
                    diag.with_note(format!(
                        "this error occurred in array dimension {}",
                        dimension + 1
                    ))
                }

                diag
            }
        }
    }
}
