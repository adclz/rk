use crate::CallSite;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_ty::resolver::visibility::SameNamespaceResult;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum VisibilityError<'db> {
    Private {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
    },
    Protected {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
    },
    Internal {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
        result: SameNamespaceResult<'db>,
    },
    /// An item reached through a `NAMESPACE INTERNAL` from outside its
    /// enclosing namespace, or a USING of one.
    InternalNamespace {
        call_site: CallSite<'db>,
        namespace: NamespaceDecl<'db>,
    },
    /// A `FUNCTION PRIVATE` called from outside its scope; the declaration
    /// is the related span.
    PrivateFunction {
        call_site: CallSite<'db>,
        target: CallSite<'db>,
    },
    /// PROTECTED or INTERNAL written on a FUNCTION header: neither has a
    /// meaning there (no class, and a namespace-wide meaning would only
    /// duplicate PRIVATE).
    SpecifierNotOnFunction {
        site: CallSite<'db>,
        keyword: &'static str,
    },
    /// Attempting to reference a {test}-annotated POU. The runner calls it;
    /// code cannot, because it is emitted with the runner's signature.
    TestOnly { call_site: CallSite<'db> },
}

impl<'db> ErrorCode for VisibilityError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::Private { .. } => "E1001",
            Self::Protected { .. } => "E1002",
            Self::Internal { .. } => "E1003",
            Self::InternalNamespace { .. } => "E1004",
            Self::PrivateFunction { .. } => "E1005",
            Self::SpecifierNotOnFunction { .. } => "E1006",
            Self::TestOnly { .. } => "E1007",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::Private { .. } => "access control violation",
            Self::Protected { .. } => "access control violation",
            Self::Internal { .. } => "access control violation",
            Self::InternalNamespace { .. } => "access control violation",
            Self::PrivateFunction { .. } => "access control violation",
            Self::SpecifierNotOnFunction { .. } => "access control violation",
            Self::TestOnly { .. } => "access control violation",
        }
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
            VisibilityError::InternalNamespace {
                call_site,
                namespace,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access '{}' from INTERNAL namespace",
                        namespace.path(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(ide_diagnostic::Related::new(
                    "namespace is declared INTERNAL here".to_string(),
                    namespace.scope_id(db).file(db),
                    CallSite::new(namespace.scope_id(db), namespace.name_id(db)).get_span(db),
                ));
                diag
            }
            VisibilityError::PrivateFunction { call_site, target } => {
                let mut diag = diag()
                    .message(format!(
                        "can not call PRIVATE function '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(ide_diagnostic::Related::new(
                    "declared PRIVATE here".to_string(),
                    target.get_scope_id(db).file(db),
                    target.get_span(db),
                ));
                diag
            }
            VisibilityError::SpecifierNotOnFunction { site, keyword } => diag()
                .message(format!(
                    "'{keyword}' does not apply to a FUNCTION: only PRIVATE does"
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                .call(),
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
                    "a {test} FUNCTION is the test runner's entry point, not a callable; \
                     for code shared between tests, write a FUNCTION without the pragma"
                        .into(),
                );

                diag
            }
        }
    }
}
