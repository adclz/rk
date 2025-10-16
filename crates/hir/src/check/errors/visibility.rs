use auto_lsp::{core::span::Span, default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};
use serde_json::to_string;

use crate::{
    check::{
        check_visibility::SameNamespaceResult,
        errors::{
            analysis_error::{AnalysisError, ToIdeDiagnostic},
            utils::get_def_for_ty,
        },
    }, hir_def::{expressions::invocation::Invocation, scope::FileScopeId}, hir_ty::{invocation_resolver::ResolvedInvocation, ty::Ty, ty_var_access_resolver::ResolvedAccess}, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum VisibilityError<'db> {
    PrivateMethod {
        method: ResolvedAccess<'db>,
        call_site: Span,
    },
    InternalMethod {
        method: ResolvedAccess<'db>,
        result: SameNamespaceResult<'db>,
        call_site: Span,
    },
    ProtectedMethod {
        method: ResolvedAccess<'db>,
        call_site: Span,
    },
}

impl<'db> From<VisibilityError<'db>> for AnalysisError<'db> {
    fn from(err: VisibilityError<'db>) -> Self {
        AnalysisError::VisibilityError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for VisibilityError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            VisibilityError::PrivateMethod { method, call_site } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access PRIVATE METHOD '{}'",
                        method.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call_site.clone())
                    .call();

                diag.with_note(
                    "METHODS marked PRIVATE can only be accessed from within the same POU".into(),
                );

                diag
            }
            VisibilityError::InternalMethod {
                method,
                result,
                call_site,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access INTERNAL METHOD '{}'",
                        method.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call_site.clone())
                    .call();

                match result {
                    SameNamespaceResult::DifferentNamespaces((ns1, ns2)) => {
                        diag.with_note(format!(
                            "calling scope is in NAMESPACE '{}', method is only available in NAMESPACE '{}'",
                            ns1.path(db).to_string(db),
                            ns2.path(db).to_string(db)
                        ));
                    }
                    SameNamespaceResult::GlobalAndNamespace(ns) => {
                        diag.with_note(format!(
                            "calling scope is in the GLOBAL scope, method is only available in NAMESPACE '{}'",
                            ns.path(db).to_string(db),
                        ));
                    }
                    SameNamespaceResult::NamespaceAndGlobal(ns) => {
                        diag.with_note(format!(
                            "calling scope is in NAMESPACE '{}', method scope is only available the GLOBAL scope",
                            ns.path(db).to_string(db),
                        ));
                    }
                    SameNamespaceResult::Same => {} // Should not happen
                }
                diag
            }
            VisibilityError::ProtectedMethod { method, call_site } => {
                let mut diag = diag()
                    .message(format!(
                        "can not access PROTECTED METHOD '{}'",
                        method.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call_site.clone())
                    .call();

                diag.with_note("METHODS marked PROTECTED are only available within the same POU or derived POUs".into());

                diag
            }
        }
    }
}
