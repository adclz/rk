use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::InitExpr;
use crate::hir_def::expressions::expression::PathExpr;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter::Range;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::diag;

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
    NonIntegerIndex {
        expr: Expr<'db>,
        ty: crate::hir_ty::ty::Type<'db>,
    },
    // Array access
    InvalidIndex {
        size: SpanIdent<'db>,
        err: String,
    },
    IndexOutOfBounds {
        expr: Expr<'db>,
        dimension: usize,
        index: i64,
        min: i64,
        max: i64,
    },
    TooManyElements {
        expr: InitExpr<'db>,
        dimension: usize,
        max_size: usize,
    },
    IndexNonArrayTypeInitExpr {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    IndexNonArrayTypePathExpr {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    ArrayConformandNotSupported(Range),
}

impl<'db> ErrorCode for ArrayError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidArrayLowerValue { .. } => "E0501",
            Self::InvalidArrayUpperValue { .. } => "E0502",
            Self::InferiorUpperBound { .. } => "E0503",
            Self::NonIntegerIndex { .. } => "E0504",
            Self::InvalidIndex { .. } => "E0505",
            Self::IndexOutOfBounds { .. } => "E0506",
            Self::TooManyElements { .. } => "E0507",
            Self::IndexNonArrayTypeInitExpr { .. } => "E0508",
            Self::IndexNonArrayTypePathExpr { .. } => "E0508",
            Self::ArrayConformandNotSupported(_) => "E0509",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InvalidArrayLowerValue { .. } => "invalid array bounds",
            Self::InvalidArrayUpperValue { .. } => "invalid array bounds",
            Self::InferiorUpperBound { .. } => "invalid array bounds",
            Self::NonIntegerIndex { .. } => "invalid array access",
            Self::InvalidIndex { .. } => "invalid array access",
            Self::IndexOutOfBounds { .. } => "invalid array access",
            Self::TooManyElements { .. } => "invalid array access",
            Self::IndexNonArrayTypeInitExpr { .. } => "invalid operation",
            Self::IndexNonArrayTypePathExpr { .. } => "invalid operation",
            Self::ArrayConformandNotSupported(_) => "syntax",
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
            Self::NonIntegerIndex { expr, ty } => diag()
                .message(format!(
                    "array index must be an integer, found {}",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::InvalidIndex { size, err } => diag()
                .message(format!("invalid index value '{}': {err}", size.text(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &size.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexOutOfBounds {
                expr,
                dimension,
                index,
                min,
                max,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "index {index} is out of bounds (the dimension is declared {min}..{max})"
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
            Self::IndexNonArrayTypeInitExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexNonArrayTypePathExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::ArrayConformandNotSupported(span) => diag()
                .message("array conformands (ARRAY[*]) are not supported".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
        }
    }
}
