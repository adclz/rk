use auto_lsp::{core::span::Span, default::db::BaseDatabase, lsp_types::{DiagnosticSeverity, DiagnosticTag}};
use ide_diagnostic::{diag, IdeDiagnostic, Related};

use crate::{check::errors::sem_errors::ToIdeDiagnostic, hir_def::{interned::namespace::NamespacePath, namespace::NamespaceDecl, using::Using}, to_proto::ToProto};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum NamespaceError<'db> {
    NamespaceNotFound {
        namespace_path: NamespacePath,
        span: Span,
    },
    NamespaceAlreadyInScope {
        using: Using<'db>,
        namespace: NamespaceDecl<'db>,
    },
    DuplicateUsing {
        using: Using<'db>,
        other: Using<'db>,
    },
}


impl<'db> ToIdeDiagnostic<'db> for NamespaceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::NamespaceNotFound {
                namespace_path,
                span,
            } => diag()
                .message(format!(
                    "namespace '{}' not found",
                    namespace_path.to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),

            Self::DuplicateUsing { using, other } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate `USING` for namespace '{}'",
                        using.path(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(using.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "namespace '{}' is already imported here",
                        other.path(db).to_string(db)
                    ),
                    other.scope_id(db).file(db),
                    other.get_span(db),
                ));

                diag
            }
            Self::NamespaceAlreadyInScope { using, namespace } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is already in scope",
                        using.path(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::WARNING)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .range(using.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "namespace '{}' is defined here",
                        using.path(db).to_string(db)
                    ),
                    namespace.scope_id(db).file(db),
                    namespace.get_span(db),
                ));

                diag
            }
        }
    }
}
