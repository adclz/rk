use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::BeginPathExpr;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::spec::Enum;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::diag;

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
    EnumValueNotConstant {
        value: Expr<'db>,
    },
}

impl<'db> ErrorCode for EnumError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidEnumType { .. } => "E0601",
            Self::NotAnEnum { .. } => "E0602",
            Self::EnumVariantNotFound { .. } => "E0603",
            Self::EnumValueNotConstant { .. } => "E0604",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InvalidEnumType { .. } => "invalid enum type",
            Self::NotAnEnum { .. } => "invalid enum access",
            Self::EnumVariantNotFound { .. } => "invalid enum access",
            Self::EnumValueNotConstant { .. } => "invalid enum value",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for EnumError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            EnumError::InvalidEnumType { value, typ } => {
                let mut diag = diag()
                    .message(format!("invalid enum type '{}'", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_note("only numeric integer types are allowed for ENUM".to_string());

                diag
            }
            Self::NotAnEnum { expr, item } => diag()
                .message(format!("'{}' is not an ENUM type", item.type_name(db)))
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
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
                .range(crate::denormalize(db, file, &variant_name.get_span(db)).unwrap_or_default())
                .call(),
            Self::EnumValueNotConstant { value } => diag()
                .message(
                    "an enum variant value must evaluate to a constant at compile time".to_string(),
                )
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                .call(),
        }
    }
}
