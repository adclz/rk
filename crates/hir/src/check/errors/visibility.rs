use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    CallSite, HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_ty::resolver::visibility::SameNamespaceResult,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum VisibilityError<'db> {
    Private {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
    },
    Internal {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
        result: SameNamespaceResult<'db>,
    },
    Protected {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
    },
}

impl<'db> From<VisibilityError<'db>> for AnalysisError<'db> {
    fn from(err: VisibilityError<'db>) -> Self {
        AnalysisError::VisibilityError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for VisibilityError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            VisibilityError::Private { call_site, target } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access PRIVATE item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call_site.get_span(db))
                    .call();

                diag.with_note(
                    "variables and methods marked PRIVATE can only be accessed from within the same POU".into(),
                );

                diag
            }
            VisibilityError::Internal {
                call_site,
                result,
                target,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access INTERNAL item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call_site.get_span(db))
                    .call();

                match result {
                    SameNamespaceResult::DifferentNamespaces((ns1, ns2)) => {
                        diag.with_note(format!(
                            "calling scope is in NAMESPACE '{}', item is only available in NAMESPACE '{}'",
                            ns1.path(db).to_string(db),
                            ns2.path(db).to_string(db)
                        ));
                    }
                    SameNamespaceResult::GlobalAndNamespace(ns) => {
                        diag.with_note(format!(
                            "calling scope is in the GLOBAL scope, item is only available in NAMESPACE '{}'",
                            ns.path(db).to_string(db),
                        ));
                    }
                    SameNamespaceResult::NamespaceAndGlobal(ns) => {
                        diag.with_note(format!(
                            "calling scope is in NAMESPACE '{}', item scope is only available the GLOBAL scope",
                            ns.path(db).to_string(db),
                        ));
                    }
                    SameNamespaceResult::Same => {} // Should not happen
                }
                diag
            }
            VisibilityError::Protected { call_site, target } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access PROTECTED item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call_site.get_span(db))
                    .call();

                diag.with_note("Variables and methods marked PROTECTED are only available within the same POU or derived POUs".into());

                diag
            }
        }
    }
}
