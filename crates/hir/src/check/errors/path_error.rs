use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    CallSite, HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
    hir_def::{
        expressions::expression::{BeginPathExpr, PathExpr},
        interned::namespace::SpanNamespaceAccess,
        scope::ScopeKind,
        semantic_index::get_scope,
        using::Using,
    },
    query_string::variables::fuzzy_variables,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AccessError<'db> {
    // Using directive
    InvalidUsingDirective { using: Using<'db> },
    NoBeginLocalItemInScope { expr: BeginPathExpr<'db> },
    NoLocalItemInScope { expr: PathExpr<'db> },
    NoItemInScope { access: SpanNamespaceAccess<'db> },
    // OOP
    ThisOnIncompatiblePou { call_site: CallSite<'db> },
    SuperOnIncompatiblePou { call_site: CallSite<'db> },
    SuperBodyOnIncompatiblePou { call_site: CallSite<'db> },
}

impl<'db> From<AccessError<'db>> for AnalysisError<'db> {
    fn from(err: AccessError<'db>) -> Self {
        AnalysisError::AccessError(err)
    }
}
impl<'db> DiagnosticDescription<'db> for AccessError<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            AccessError::InvalidUsingDirective { using } => {
                format!(
                    "could not find namespace '{}'",
                    using.path(db).to_string(db)
                )
            }
            AccessError::NoBeginLocalItemInScope { expr } => {
                format!("no begin item '{}' in scope", expr.to_string(db))
            }
            AccessError::NoLocalItemInScope { expr } => {
                format!("no item '{}' in scope", expr.ident(db).text(db))
            }
            AccessError::NoItemInScope { access } => {
                format!("no path or item '{}' in scope", access.to_string(db))
            }
            // OOP
            AccessError::ThisOnIncompatiblePou { call_site } => {
                "'THIS' is not valid in this context".to_string()
            }
            AccessError::SuperOnIncompatiblePou { call_site } => {
                "'SUPER' is not valid in this context".to_string()
            }
            AccessError::SuperBodyOnIncompatiblePou { call_site } => {
                "'SUPER()' is not valid in this context".to_string()
            }
        }
    }

    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match self {
            AccessError::NoBeginLocalItemInScope { expr } => {}
            AccessError::NoLocalItemInScope { expr } => {
                let scope = get_scope(db, expr.scope_id(db));
                if let ScopeKind::Pou(pou) = scope.kind {
                    fuzzy_variables(db, pou, diag, expr.ident(db).as_str(db))
                }
            }
            AccessError::NoItemInScope { access } => {}
            _ => {}
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for AccessError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        let mut diag = diag()
            .message(self.description(db))
            .severity(DiagnosticSeverity::ERROR)
            .range(match self {
                AccessError::InvalidUsingDirective { using } => using.get_span(db),
                AccessError::NoBeginLocalItemInScope { expr } => expr.get_span(db),
                AccessError::NoLocalItemInScope { expr } => expr.get_span(db),
                AccessError::NoItemInScope { access } => access.get_span(db),
                AccessError::ThisOnIncompatiblePou { call_site } => call_site.get_span(db),
                AccessError::SuperOnIncompatiblePou { call_site } => call_site.get_span(db),
                AccessError::SuperBodyOnIncompatiblePou { call_site } => call_site.get_span(db),
            })
            .call();

        self.related(db, &mut diag);
        diag
    }
}
