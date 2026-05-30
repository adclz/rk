use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    CallSite, HirNodeInfo, check::errors::ToIdeDiagnostic,
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
    /// Attempting to reference a {test}-annotated POU from non-test code.
    TestOnly { call_site: CallSite<'db> },
}

impl ErrorCode for VisibilityError<'_> {
    fn code(&self) -> &'static str {
        match self {
            Self::Private { .. } => "E0401",
            Self::Internal { .. } => "E0402",
            Self::Protected { .. } => "E0403",
            Self::TestOnly { .. } => "E0404",
        }
    }

    fn description(&self) -> &'static str {
        "access control violation"
    }
}

impl<'db> ToIdeDiagnostic<'db> for VisibilityError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            VisibilityError::Private { call_site, target } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access PRIVATE item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
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
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
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
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note("Variables and methods marked PROTECTED are only available within the same POU or derived POUs".into());

                diag
            }
            VisibilityError::TestOnly { call_site } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access test item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note(
                    "items marked with {test} can only be referenced from other {test} items"
                        .into(),
                );

                diag
            }
        }
    }
}
