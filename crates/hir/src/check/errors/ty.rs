use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HirNodeInfo, TypeInfo,
    check::errors::{
        analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
        coerce::{ExprMismatch, TypeMismatch},
    },
    hir_def::interned::{identifier::SpanIdent, namespace::SpanNamespaceAccess},
    hir_ty::{expr_resolver::ResolvedExpr, ty::Ty},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyError<'db> {
    Recursive {
        origin: Ty<'db>,
    },
    ReferenceRecursive {
        origin: Ty<'db>,
        target: Ty<'db>,
    },
    UnresolvedNamespace {
        ty: Ty<'db>,
        path: SpanNamespaceAccess<'db>,
    },
    // Arrays
    InvalidArrayLowerValue {
        value: ResolvedExpr<'db>,
    },
    InvalidArrayUpperValue {
        value: ResolvedExpr<'db>,
    },
    InferiorUpperBound {
        lower: u64,
        upper: u64,
        upper_expr: ResolvedExpr<'db>,
    },
    // Enums
    InvalidEnumType {
        value: Ty<'db>,
    },
    InvalidEnumVariantValue {
        variant: SpanIdent<'db>,
        err: ExprMismatch<'db>,
    },
    // Subrange
    InvalidSubrangeType {
        value: Ty<'db>,
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

impl<'db> From<TyError<'db>> for AnalysisError<'db> {
    fn from(err: TyError<'db>) -> Self {
        AnalysisError::TyError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for TyError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            TyError::UnresolvedNamespace { ty, path } => diag()
                .message(format!("unknown item '{}'", path.path.to_string(db)))
                .severity(DiagnosticSeverity::ERROR)
                .range(path.get_span(db))
                .call(),
            TyError::Recursive { origin } => diag()
                .message(format!(
                    "'{}' is recursive",
                    origin.name(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(origin.name_span(db))
                .call(),
            TyError::ReferenceRecursive { origin, target } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' creates a recursion with '{}'",
                        origin.name(db),
                        target.name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(origin.get_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "'{}' is originally declared here",
                        target.name(db)
                    ),
                    target.get_scope_id(db).file(db),
                    target.name_span(db),
                ));

                diag.with_related(Related::new(
                    "... and recurse at this location".to_string(),
                    origin.get_scope_id(db).file(db),
                    origin.name_span(db),
                ));

                diag
            }
            TyError::InvalidArrayLowerValue { value } => diag()
                .message("Invalid lower bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(value.expr(db).get_span(db))
                .call(),
            TyError::InvalidArrayUpperValue { value } => diag()
                .message("Invalid upper bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(value.expr(db).get_span(db))
                .call(),
            TyError::InferiorUpperBound {
                lower,
                upper,
                upper_expr,
            } => diag()
                .message("Upper bound value must be greater than lower bound value".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(upper_expr.get_span(db))
                .call(),
            TyError::InvalidEnumType { value } => {
                let mut diag = diag()
                    .message(format!("invalid enum type '{}'", value.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(value.name_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for ENUM".to_string());

                diag
            }
            TyError::InvalidEnumVariantValue { variant, err } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid value for enum variant '{}': {}",
                        variant.ident.text(db),
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(variant.get_span(db))
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);

                diag
            }
            TyError::InvalidSubrangeType { value } => {
                let mut diag = diag()
                    .message(format!("invalid subrange type '{}'", value.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(value.name_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for SUBRANGE".to_string());

                diag
            }
            TyError::InvalidSubrangeStart { expr, err } => {
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
            TyError::InvalidSubrangeEnd { expr, err } => {
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
