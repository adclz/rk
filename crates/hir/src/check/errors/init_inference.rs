
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, body_inference::TypeError},
    hir_def::{
        expressions::expression::InitExpr,
        interned::identifier::{Ident, SpanIdent},
    },
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
    InvalidIndex {
        size: SpanIdent<'db>,
        err: String,
    },
    IndexOutOfBounds {
        size: SpanIdent<'db>,
        dimension: usize,
        index: u64,
        min: u64,
        max: u64,
    },
    TypeMismatch(TypeError<'db>),
    TooManyElements {
        expr: InitExpr<'db>,
        dimension: usize,
        max_size: usize,
    },
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
            Self::InvalidIndex { size, err } => diag()
                .message(format!("invalid index value '{}': {err}", size.text(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .range(size.get_span(db))
                .call(),
            Self::IndexOutOfBounds {
                size,
                dimension,
                index,
                min,
                max,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "index '{}' is out of bounds (expected between {} and {})",
                        index, min, max
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .range(size.get_span(db))
                    .call();

                if *dimension > 0_usize {
                    diag.with_note(format!(
                        "this error occurred in array dimension {}",
                        dimension + 1
                    ))
                }

                diag
            }
            Self::TooManyElements {
                expr,
                dimension,
                max_size,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "too many elements in array initializer (expected at most {})",
                        max_size
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db))
                    .call();

                if *dimension > 0_usize {
                    diag.with_note(format!(
                        "this error occurred in array dimension {}",
                        dimension + 1
                    ))
                }

                diag
            }
            Self::TypeMismatch(mismatch) => mismatch.to_diagnostic(db),
        }
    }
}
