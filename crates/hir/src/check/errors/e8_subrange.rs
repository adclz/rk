use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::expressions::{expression::Expr, spec::Spec},
    hir_ty::ty::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SubRangeError<'db> {
    // Subrange
    InvalidSubrangeType { spec: Spec<'db>, typ: Type<'db> },
    /// A constant assigned to a subrange variable lies outside its bounds.
    ValueOutOfRange {
        expr: Expr<'db>,
        value: i64,
        lower: i64,
        upper: i64,
    },
    /// A subrange bound must evaluate to a constant at compile time.
    BoundNotConstant { value: Expr<'db> },
}

impl<'db> ErrorCode for SubRangeError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidSubrangeType { .. } => "E0801",
            Self::ValueOutOfRange { .. } => "E0802",
            Self::BoundNotConstant { .. } => "E0803",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InvalidSubrangeType { .. } => "invalid subrange type",
            Self::ValueOutOfRange { .. } => "value outside subrange",
            Self::BoundNotConstant { .. } => "invalid subrange bound",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for SubRangeError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            SubRangeError::InvalidSubrangeType { spec, typ } => {
                let mut diag = diag()
                    .message(format!("Invalid subrange type '{}'", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &spec.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_note("only numeric integer types are allowed for SUBRANGE".to_string());

                diag
            }
            SubRangeError::BoundNotConstant { value } => diag()
                .message(
                    "a subrange bound must evaluate to a constant at compile time".to_string(),
                )
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                .call(),
            SubRangeError::ValueOutOfRange {
                expr,
                value,
                lower,
                upper,
            } => {
                let mut diag = diag()
                    .message(format!("value {value} is outside the subrange {lower}..{upper}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_note(format!(
                    "the declared range only admits values from {lower} to {upper}"
                ));

                diag
            }
        }
    }
}
