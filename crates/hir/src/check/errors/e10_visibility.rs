// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    /// code cannot, because a test FUNCTION is emitted with the runner's
    /// signature, and a test FUNCTION_BLOCK runs on the runner's instance,
    /// left out of the build of a library's user with the library's tests.
    TestOnly { call_site: CallSite<'db> },
    /// What a library's code names that it does not see: another library's
    /// declaration, the program's, or a global. A library uses itself and
    /// the standard library only, whatever the program that reads it lists.
    OutsideTheLibrary {
        /// What the code wrote.
        text: compact_str::CompactString,
        span: auto_lsp::tree_sitter::Range,
        reached: Reached,
    },
}

/// What a library's code reached for outside it (E1008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub enum Reached {
    /// A declaration of the library of this name.
    Library(crate::hir_def::interned::identifier::Ident),
    /// A declaration of the program that reads the library.
    Program,
    /// A global, which only the program declares: by a VAR_EXTERNAL, or by
    /// its bare name.
    Global,
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
            Self::OutsideTheLibrary { .. } => "E1008",
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
                        "cannot access PRIVATE item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note(
                    "a PRIVATE variable or method is reached from its own POU only".into(),
                );

                diag
            }
            VisibilityError::Protected { call_site, target } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot access PROTECTED item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note(
                    "a PROTECTED variable or method is reached from its own POU and those derived from it".into(),
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
                        "cannot access INTERNAL item '{}'",
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
                            "the item is INTERNAL to NAMESPACE '{}', while this is NAMESPACE '{}'",
                            ns2.path_with_case(db).to_string(db),
                            ns1.path_with_case(db).to_string(db)
                        ));
                    }
                    SameNamespaceResult::GlobalAndNamespace(ns) => {
                        diag.with_note(format!(
                            "the item is INTERNAL to NAMESPACE '{}', while this is the global scope",
                            ns.path_with_case(db).to_string(db),
                        ));
                    }
                    SameNamespaceResult::NamespaceAndGlobal(ns) => {
                        diag.with_note(format!(
                            "the item is INTERNAL to the global scope, while this is NAMESPACE '{}'",
                            ns.path_with_case(db).to_string(db),
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
                        "cannot access INTERNAL namespace '{}'",
                        namespace.path_with_case(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(ide_diagnostic::Related::new(
                    "the namespace is declared INTERNAL here".to_string(),
                    namespace.scope_id(db).file(db),
                    CallSite::new(namespace.scope_id(db), namespace.name_id(db)).get_span(db),
                ));
                diag
            }
            VisibilityError::PrivateFunction { call_site, target } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot call PRIVATE function '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(ide_diagnostic::Related::new(
                    "the FUNCTION is declared PRIVATE here".to_string(),
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
                        "cannot access test item '{}'",
                        call_site.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note(
                    "a {test} POU is the test runner's entry point, not a callable".into(),
                );
                diag.with_help(
                    "for code shared between tests, write a POU without the pragma".into(),
                );

                diag
            }
            VisibilityError::OutsideTheLibrary {
                text,
                span,
                reached,
            } => {
                let message = match reached {
                    Reached::Library(name) => {
                        format!("'{text}' is declared by the library '{}'", name.text(db))
                    }
                    Reached::Program => format!("'{text}' is declared by the program"),
                    Reached::Global => format!("'{text}' is a global"),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                match reached {
                    Reached::Global => {
                        diag.with_note(
                            "a library sees no global: the program declares them".into(),
                        );
                        diag.with_help("take it as a VAR_IN_OUT".into());
                    }
                    _ => diag.with_note(
                        "a library sees its own declarations and the standard library's".into(),
                    ),
                }
                diag
            }
        }
    }
}
