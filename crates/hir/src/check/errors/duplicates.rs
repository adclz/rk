use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic}, hir_def::interned::identifier::SpanIdent, hir_ty::{inheritance_solver::InheritedMethod, ty::Ty}, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Pou {
        pou1: Ty<'db>,
        pou2: Ty<'db>,
    },
    Variable {
        var1: Ty<'db>,
        var2: Ty<'db>,
    },
    StructField {
        field1: Ty<'db>,
        field2: Ty<'db>,
    },
    EnumVariant {
        variant1: SpanIdent<'db>,
        variant2: SpanIdent<'db>,
    },
    Method {
        method1: Ty<'db>,
        method2: Ty<'db>,
    },
    InheritedMethod {
        method1: InheritedMethod<'db>,
        method2: InheritedMethod<'db>,
    },
}

impl<'db> From<DuplicateError<'db>> for AnalysisError<'db> {
    fn from(err: DuplicateError<'db>) -> Self {
        AnalysisError::DuplicateError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::Pou { pou1, pou2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate POU '{}'",
                        pou1.name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou1.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "POU '{}' is already defined here",
                        pou2.name(db)
                    ),
                    pou2.get_scope_id(db).file(db),
                    pou2.name_span(db),
                ));

                diag
            }
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate variable '{}'",
                        var1.name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var1.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variable '{}' is already defined here",
                        var2.name(db)
                    ),
                    var2.get_scope_id(db).file(db),
                    var2.name_span(db),
                ));

                diag
            }
            Self::EnumVariant { variant1, variant2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate enum variant '{}'",
                        variant1.ident.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(variant1.get_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "enum variant '{}' is already defined here",
                        variant2.ident.text(db)
                    ),
                    variant2.get_scope_id(db).file(db),
                    variant2.get_span(db),
                ));

                diag
            }
            Self::StructField { field1, field2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate field '{}'",
                        field1.name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(field1.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "field '{}' is already defined here",
                        field2.name(db)
                    ),
                    field2.get_scope_id(db).file(db),
                    field2.name_span(db),
                ));

                diag
            }
            Self::Method { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method1.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.name(db)
                    ),
                    method2.get_scope_id(db).file(db),
                    method2.name_span(db),
                ));

                diag
            }
            Self::InheritedMethod { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method1.method.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.method.name(db).text(db)
                    ),
                    method2.method.get_scope_id(db).file(db),
                    method2.method.name_span(db),
                ));

                diag.with_note(format!(
                    "this error happens because both interfaces '{}' and '{}' define a method '{}'",
                    method1.source.name(db).text(db),
                    method2.source.name(db).text(db),
                    method1.method.name(db).text(db)
                ));

                diag
            }
        }
    }
}
