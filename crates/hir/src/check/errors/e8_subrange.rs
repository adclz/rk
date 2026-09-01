use auto_lsp::{lsp_types::DiagnosticSeverity, tree_sitter};
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
    /// A by-reference binding (VAR_IN_OUT, or an `=>` output destination)
    /// whose two ends disagree about the subrange. The callee writes through
    /// its OWN declared type, so a disagreement is a door around the range
    /// check: an INT param scribbling 99 into the caller's `INT (0..10)`.
    ByRefSubrangeMismatch {
        span: tree_sitter::Range,
        param: Type<'db>,
        arg: Type<'db>,
    },
}

impl<'db> ErrorCode for SubRangeError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidSubrangeType { .. } => "E0801",
            Self::ValueOutOfRange { .. } => "E0802",
            Self::BoundNotConstant { .. } => "E0803",
            Self::ByRefSubrangeMismatch { .. } => "E0804",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InvalidSubrangeType { .. } => "invalid subrange type",
            Self::ValueOutOfRange { .. } => "value outside subrange",
            Self::BoundNotConstant { .. } => "invalid subrange bound",
            Self::ByRefSubrangeMismatch { .. } => "subrange mismatch across a reference",
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
            SubRangeError::ByRefSubrangeMismatch { span, param, arg } => diag()
                .message(format!(
                    "'{}' binds by reference to '{}': the subrange must match exactly",
                    with_bounds(db, *arg),
                    with_bounds(db, *param),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
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

/// A type with its subrange spelled out, so a mismatch between two
/// similarly-named subranges says WHICH constraint differs:
/// `Small (0..10)` against `Smaller (0..20)`.
pub(crate) fn with_bounds<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> String {
    let name = ty.type_name(db);
    match ty.as_subrange(db) {
        Some(sub) => {
            let (lower, upper) = crate::hir_ty::infer::const_eval::subrange_bounds(db, sub);
            match (lower, upper) {
                // An anonymous `INT (0..10)` already names its bounds.
                (Some(l), Some(u)) if !name.ends_with(&format!("({l}..{u})")) => {
                    format!("{name} ({l}..{u})")
                }
                _ => name.to_string(),
            }
        }
        None => name.to_string(),
    }
}