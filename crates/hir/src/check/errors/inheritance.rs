use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{diag, IdeDiagnostic};

use crate::{check::errors::{sem_errors::{AnalysisError, ToIdeDiagnostic}, utils::get_decl_for_ty}, hir_ty::ty::Ty};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MethodError<'db> {
    OverrideFinalMethod {
        base_method: Ty<'db>,
        derived_method: Ty<'db>,
    },
    MissingOverride {
        base_method: Ty<'db>,
        derived_method: Ty<'db>,
    },
    MissingAbstractMethod {
        implementer: Ty<'db>,
        base_method: Ty<'db>,
    },
    EmptyOverride {
        base_method: Ty<'db>,
    },
    AbstractClassHasNoAbstractMethods {
        class: Ty<'db>,
    },
    UnimplementedInterfaceMethod {
        implementer: Ty<'db>,
        method: Ty<'db>,
    },
}

impl<'db> From<MethodError<'db>> for AnalysisError<'db> {
    fn from(err: MethodError<'db>) -> Self {
        AnalysisError::MethodError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for MethodError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::MissingOverride {
                base_method,
                derived_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing OVERRIDE keyword for method '{}'",
                        derived_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *base_method, &mut diag);
                diag.with_note("OVERRIDE keyword must be used even if the base method is not marked as ABSTRACT".into());

                diag
            }
            Self::OverrideFinalMethod {
                base_method,
                derived_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot override FINAL method '{}'",
                        base_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *base_method, &mut diag);
                diag.with_note("methods marked as FINAL cannot be overridden".into());

                diag
            }
            Self::MissingAbstractMethod {
                implementer,
                base_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing implementation for ABSTRACT method '{}'",
                        base_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *base_method, &mut diag);
                diag.with_note("ABSTRACT methods must be implemented by derived POUs".into());

                diag
            }
            Self::EmptyOverride { base_method } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid usage of OVERRIDE for method '{}'",
                        base_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(base_method.decl(db).name_span(db))
                    .call();

                diag.with_note("OVERRIDE is only valid when the method is inherited".into());

                diag
            }
            Self::AbstractClassHasNoAbstractMethods { class } => {
                let mut diag = diag()
                    .message(format!(
                        "ABSTRACT class '{}' has no abstract methods",
                        class.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(class.decl(db).name_span(db))
                    .call();

                diag.with_note("abstract classes must have at least one abstract method".into());

                diag
            }
            Self::UnimplementedInterfaceMethod {
                implementer,
                method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing implementation for interface method '{}'",
                        method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *method, &mut diag);

                diag
            }
        }
    }
}
