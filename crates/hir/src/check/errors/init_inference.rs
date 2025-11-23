use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, body_inference::TypeError},
    hir_def::{
        expressions::expression::InitExpr,
        interned::identifier::{Ident, SpanIdent},
    },
    hir_ty::ty::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitInferenceError<'db> {
    NoSuchField {
        expr: InitExpr<'db>,
        ident: Ident,
        ty: Type<'db>,
    },
    IsElementaryType {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    IndexNonArrayType {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    TypeMismatch(TypeError<'db>),
}

impl<'db> From<TypeError<'db>> for InitInferenceError<'db> {
    fn from(value: TypeError<'db>) -> Self {
        InitInferenceError::TypeMismatch(value)
    }
}

impl<'db> ToIdeDiagnostic<'db> for InitInferenceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::IndexNonArrayType { expr, ty } => {
                let mut diag = ide_diagnostic::diag()
                    .message(format!("cannot index into type '{}'", ty.type_name(db)))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db))
                    .call();

                diag
            }
            Self::IsElementaryType { expr, ty } => {
                let mut diag = ide_diagnostic::diag()
                    .message(format!(
                        "type '{}' is an elementary type and cannot be initiliazed with '()'",
                        ty.type_name(db)
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db))
                    .call();

                diag
            }
            Self::NoSuchField { expr, ident, ty } => {
                let mut diag = ide_diagnostic::diag()
                    .message(format!(
                        "no field '{}' in type '{}'",
                        ident.text(db),
                        ty.type_name(db)
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db))
                    .call();

                diag
            }
            Self::TypeMismatch(mismatch) => mismatch.to_diagnostic(db),
        }
    }
}
