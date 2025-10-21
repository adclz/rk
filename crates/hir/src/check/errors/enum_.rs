use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    check::errors::{
        analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
        coerce::{ExprMismatch, TypeMismatch},
    }, hir_def::{expressions::spec::Spec, interned::{identifier::SpanIdent, namespace::SpanNamespaceAccess}}, hir_ty::{expr_resolver::ResolvedExpr, ty::Ty}, HirNodeInfo, TypeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum EnumError<'db> {
    // Enums
    InvalidEnumType {
        value: Spec<'db>,
    },
    InvalidEnumVariantValue {
        variant: SpanIdent<'db>,
        err: ExprMismatch<'db>,
    },
}

impl<'db> From<EnumError<'db>> for AnalysisError<'db> {
    fn from(err: EnumError<'db>) -> Self {
        AnalysisError::EnumError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for EnumError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            EnumError::InvalidEnumType { value } => {
                let mut diag = diag()
                    .message(format!("invalid enum type '{}'", value.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(value.get_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for ENUM".to_string());

                diag
            }
            EnumError::InvalidEnumVariantValue { variant, err } => {
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
        }
    }
}
