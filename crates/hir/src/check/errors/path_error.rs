use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    check::errors::analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic}, hir_def::{
        expressions::expression::{BeginPathExpr, PathExpr},
        interned::namespace::SpanNamespaceAccess,
        scope::ScopeKind,
        semantic_index::{get_scope, semantic_index},
    }, hir_ty::{ty_var_access_resolver::CallSite, walk::ResolvedPath}, query_string::fuzzy_pou::fuzzy_pou_items, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AccessError<'db> {
    NoBeginLocalItemInScope {
        expr: BeginPathExpr<'db>,
    },
    NoLocalItemInScope {
        expr: PathExpr<'db>,
    },
    NoItemInScope {
        access: SpanNamespaceAccess<'db>,
    },
    InvalidTypeAccess {
        access: ResolvedPath<'db>,
    },
    UnknownField {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    TypeHasNoField {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    NotAnArray {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    NotAReference {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    UnknownMethod {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    // OOP
    ThisOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperBodyOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
}

impl<'db> From<AccessError<'db>> for AnalysisError<'db> {
    fn from(err: AccessError<'db>) -> Self {
        AnalysisError::AccessError(err)
    }
}
impl<'db> DiagnosticDescription<'db> for AccessError<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            AccessError::NoBeginLocalItemInScope { expr } => {
                format!("no begin item '{}' in scope", expr.to_string(db))
            }
            AccessError::NoLocalItemInScope { expr } => {
                format!("no item '{}' in scope", expr.ident(db).text(db))
            }
            AccessError::NoItemInScope { access } => {
                format!("no path or item '{}' in scope", access.to_string(db))
            }
            AccessError::InvalidTypeAccess { access } => {
                format!("no type found for '{}'", access.decl_name(db))
            }
            AccessError::UnknownField { ty, expr } => {
                format!(
                    "field '{}' not found in '{}'",
                    expr.ident(db).text(db),
                    ty.decl_name(db)
                )
            }
            AccessError::TypeHasNoField { ty, expr } => {
                format!("type '{}' does not have fields", ty.decl_name(db))
            }
            AccessError::NotAReference { ty, expr } => {
                format!("type '{}' can not be dereferenced", ty.decl_name(db))
            }
            AccessError::NotAnArray { ty, expr } => {
                format!("type '{}' cannot be indexed", ty.decl_name(db))
            }
            AccessError::UnknownMethod { ty, expr } => {
                format!(
                    "method '{}' not found in '{}'",
                    expr.ident(db).text(db),
                    ty.decl_name(db)
                )
            }
            // OOP
            AccessError::ThisOnIncompatiblePou { call_site } => {
                format!("'THIS' is not valid in this context")
            }
            AccessError::SuperOnIncompatiblePou { call_site } => {
                format!("'SUPER' is not valid in this context")
            }
            AccessError::SuperBodyOnIncompatiblePou { call_site } => {
                format!("'SUPER()' is not valid in this context")
            }
        }
    }

    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match self {
            AccessError::NoBeginLocalItemInScope { expr } => {

            }
            AccessError::NoLocalItemInScope { expr } => {
                let scope = get_scope(db, expr.scope_id(db));
                if let ScopeKind::Pou(pou) = scope.kind {
                    fuzzy_pou_items(
                        db,
                        pou,
                        diag,
                        expr.ident(db).as_str(db),
                    )
                }
            }
            AccessError::NoItemInScope { access } => {}
            AccessError::TypeHasNoField { ty, expr } => {
                ty.diag_with_location(db, diag);
            }
            AccessError::UnknownField { ty, expr } => {
                ty.diag_with_location(db, diag);
            }
            AccessError::NotAnArray { ty, expr } => {
                ty.diag_with_location(db, diag);
            }
            AccessError::NotAReference { ty, expr } => {
                ty.diag_with_location(db, diag);
            }
            AccessError::InvalidTypeAccess { access } => {
                access.diag_with_location(db, diag);
            },
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
                AccessError::NoBeginLocalItemInScope { expr } => expr.get_span(db),
                AccessError::NoLocalItemInScope { expr } => expr.get_span(db),
                AccessError::NoItemInScope { access } => access.get_span(db),
                AccessError::InvalidTypeAccess { access } => access.expr.get_span(db),
                AccessError::UnknownField { expr, .. } => expr.get_span(db),
                AccessError::TypeHasNoField { expr, .. } => expr.get_span(db),
                AccessError::NotAnArray { expr, .. } => expr.get_span(db),
                AccessError::NotAReference { expr, .. } => expr.get_span(db),
                AccessError::UnknownMethod { expr, .. } => expr.get_span(db),
                AccessError::ThisOnIncompatiblePou { call_site } => call_site.get_span(db),
                AccessError::SuperOnIncompatiblePou { call_site } => call_site.get_span(db),
                AccessError::SuperBodyOnIncompatiblePou { call_site } => call_site.get_span(db)
            })
            .call();

        self.related(db, &mut diag);
        diag
    }
}
