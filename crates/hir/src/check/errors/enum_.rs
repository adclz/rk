use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::expressions::spec::Spec,
    hir_ty::ty::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum EnumError<'db> {
    // Enums
    InvalidEnumType { value: Spec<'db>, typ: Type<'db> },
    /*InvalidEnumVariantValue {
        variant: SpanIdent<'db>,
        err: ExprMismatch<'db>,
    },*/
}

impl<'db> From<EnumError<'db>> for AnalysisError<'db> {
    fn from(err: EnumError<'db>) -> Self {
        AnalysisError::EnumError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for EnumError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            EnumError::InvalidEnumType { value, typ } => {
                let mut diag = diag()
                    .message(format!("invalid enum type '{}'", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(value.get_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for ENUM".to_string());

                diag
            } /*EnumError::InvalidEnumVariantValue { variant, err } => {
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
              }*/
        }
    }
}
