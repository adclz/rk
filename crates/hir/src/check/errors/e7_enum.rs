use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::BeginPathExpr,
            spec::{Enum, Spec},
        },
        interned::identifier::SpanIdent,
    },
    hir_ty::ty::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum EnumError<'db> {
    // Enums
    InvalidEnumType {
        value: Spec<'db>,
        typ: Type<'db>,
    },
    NotAnEnum {
        expr: BeginPathExpr<'db>,
        item: Type<'db>,
    },
    EnumVariantNotFound {
        enum_: Enum<'db>,
        variant_name: SpanIdent<'db>,
    },
}

impl<'db> ErrorCode for EnumError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidEnumType { .. } => "E0701",
            Self::NotAnEnum { .. } => "E0702",
            Self::EnumVariantNotFound { .. } => "E0703",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InvalidEnumType { .. } => "invalid enum type",
            _ => "invalid enum access",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for EnumError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            EnumError::InvalidEnumType { value, typ } => {
                let mut diag = diag()
                    .message(format!("I=invalid enum type '{}'", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(value.get_span(db))
                    .call();

                diag.with_note("only numeric integer types are allowed for ENUM".to_string());

                diag
            }
            Self::NotAnEnum { expr, item } => diag()
                .message(format!("'{}' is not an ENUM type", item.type_name(db)))
                .range(expr.get_span(db))
                .desc(self)
                .call(),
            Self::EnumVariantNotFound {
                enum_,
                variant_name,
            } => diag()
                .message(format!(
                    "ENUM has no variant named '{}'",
                    variant_name.text(db)
                ))
                .desc(self)
                .range(variant_name.get_span(db))
                .call(),
        }
    }
}
