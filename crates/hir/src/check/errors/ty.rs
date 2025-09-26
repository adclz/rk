use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::interned::namespace::SpanNamespaceAccess,
    hir_ty::{expr_resolver::ResolvedExpr, ty::Ty},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyError<'db> {
    Recursive {
        origin: Ty<'db>,
    },
    ReferenceRecursive {
        origin: Ty<'db>,
        target: Ty<'db>,
    },
    UnresolvedNamespace {
        ty: Ty<'db>,
        path: SpanNamespaceAccess<'db>,
    },
    InvalidArrayLowerValue {
        value: ResolvedExpr<'db>,
    },
    InvalidArrayUpperValue {
        value: ResolvedExpr<'db>,
    },
    InferiorUpperBound {
        lower: u64,
        upper: u64,
        upper_expr: ResolvedExpr<'db>,
    },
}

impl<'db> From<TyError<'db>> for AnalysisError<'db> {
    fn from(err: TyError<'db>) -> Self {
        AnalysisError::TyError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for TyError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            TyError::UnresolvedNamespace { ty, path } => diag()
                .message(format!("unknown item '{}'", path.path.to_string(db)))
                .severity(DiagnosticSeverity::ERROR)
                .range(path.get_span(db))
                .call(),
            TyError::Recursive { origin } => diag()
                .message(format!(
                    "'{}' is recursive",
                    origin.decl(db).name(db).text(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(origin.decl(db).name_span(db))
                .call(),
            TyError::ReferenceRecursive { origin, target } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' creates a recursion with '{}'",
                        origin.decl(db).name(db).text(db),
                        target.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(origin.get_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "'{}' is originally declared here",
                        target.decl(db).name(db).text(db)
                    ),
                    target.decl(db).scope_id(db).file(db),
                    target.decl(db).name_span(db),
                ));

                diag.with_related(Related::new(
                    "... and recurse at this location".to_string(),
                    origin.decl(db).scope_id(db).file(db),
                    origin.decl(db).name_span(db),
                ));

                diag
            }
            TyError::InvalidArrayLowerValue { value } => diag()
                .message("Invalid lower bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(value.expr(db).get_span(db))
                .call(),
            TyError::InvalidArrayUpperValue { value } => diag()
                .message("Invalid upper bound value for ARRAY".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(value.expr(db).get_span(db))
                .call(),
            TyError::InferiorUpperBound {
                lower,
                upper,
                upper_expr,
            } => diag()
                .message("Upper bound value must be greater than lower bound value".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(upper_expr.get_span(db))
                .call(),
        }
    }
}
