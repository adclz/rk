use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, body_inference::TypeError},
    hir_def::{expressions::expression::InitExpr, interned::identifier::Ident},
    hir_ty::ty::Type,
    query_string::strukt::fuzzy_struct_fields,
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
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::IndexNonArrayType { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .range(expr.get_span(db))
                .call(),
            Self::IsElementaryType { expr, ty } => ide_diagnostic::diag()
                .message(format!(
                    "type '{}' is an elementary type and cannot be initiliazed with '()'",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .range(expr.get_span(db))
                .call(),
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

                if let Type::Struct(strukt) = ty {
                    fuzzy_struct_fields(db, *strukt, &mut diag, ident.text(db).as_str())
                };

                diag
            }
            Self::TypeMismatch(mismatch) => mismatch.to_diagnostic(db),
        }
    }
}
