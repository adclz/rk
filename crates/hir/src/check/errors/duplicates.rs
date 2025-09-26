use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_ty::ty::Ty,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Pou { pou1: Ty<'db>, pou2: Ty<'db> },
    Variable { var1: Ty<'db>, var2: Ty<'db> },
    StructField { field1: Ty<'db>, field2: Ty<'db> },
    Method { method1: Ty<'db>, method2: Ty<'db> },
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
                        pou1.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou1.decl(db).name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "POU '{}' is already defined here",
                        pou2.decl(db).name(db).text(db)
                    ),
                    pou2.get_scope_id(db).file(db),
                    pou2.decl(db).name_span(db),
                ));

                diag
            }
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate variable '{}'",
                        var1.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var1.decl(db).name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variable '{}' is already defined here",
                        var2.decl(db).name(db).text(db)
                    ),
                    var2.get_scope_id(db).file(db),
                    var2.decl(db).name_span(db),
                ));

                diag
            }
            Self::StructField { field1, field2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate field '{}'",
                        field1.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(field1.decl(db).name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "field '{}' is already defined here",
                        field2.decl(db).name(db).text(db)
                    ),
                    field2.get_scope_id(db).file(db),
                    field2.decl(db).name_span(db),
                ));

                diag
            }
            Self::Method { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method1.decl(db).name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.decl(db).name(db).text(db)
                    ),
                    method2.get_scope_id(db).file(db),
                    method2.decl(db).name_span(db),
                ));

                diag
            }
        }
    }
}
