use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::errors::{
            analysis_error::DiagnosticDescription,
            literals::LiteralErrorKind,
            path_error::PathResolveError,
            utils::get_decl_and_def_for_ty,
            var_error::VarResolveError,
        }, hir_def::interned::identifier::SpanIdent, hir_ty::{expr_resolver::ResolvedExpr, ty::Ty}, TypeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct TypeMismatch<'db> {
    pub ty1: Ty<'db>,
    pub ty2: Ty<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ExprMismatch<'db> {
    pub expr: ResolvedExpr<'db>,
    pub kind: ExprMismatchKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExprMismatchKind<'db> {
    Literal {
        ty: Ty<'db>,
        literal: LiteralErrorKind,
    },
    TypeMismatch {
        err: TypeMismatch<'db>,
    },
    ExprTypeMismatch {
        ty: Ty<'db>,
    },
    VoidRhs {
        ty: Ty<'db>,
    },
    UnresolvedPathError {
        err: PathResolveError<'db>,
    },
    UnresolvedVarError {
        err: VarResolveError<'db>,
    },
    LhsIsNotABool {
        ty: Ty<'db>,
    },
    InvalidEnumVariant {
        enum_ty: Ty<'db>,
        variant: SpanIdent<'db>,
    },
    SubRangeValueOutOfBounds {
        expr: ResolvedExpr<'db>,
        subrange_ty: Ty<'db>,
        min: u64,
        max: u64,
        value: u64,
    },
}

impl<'db> ExprMismatch<'db> {
    pub fn type_mismatch(expr: ResolvedExpr<'db>, type_error: TypeMismatch<'db>) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::TypeMismatch { err: type_error },
        }
    }

    pub fn literal(expr: ResolvedExpr<'db>, ty: Ty<'db>, literal: LiteralErrorKind) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::Literal { literal, ty },
        }
    }

    pub fn expr_mismatch(expr: ResolvedExpr<'db>, ty: Ty<'db>) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::ExprTypeMismatch { ty },
        }
    }

    pub fn expr_void(expr: ResolvedExpr<'db>, ty: Ty<'db>) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::VoidRhs { ty },
        }
    }

    pub fn unresolved_path(expr: ResolvedExpr<'db>, err: PathResolveError<'db>) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::UnresolvedPathError { err },
        }
    }

    pub fn unresolved_var(expr: ResolvedExpr<'db>, err: VarResolveError<'db>) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::UnresolvedVarError { err },
        }
    }

    pub fn lhs_is_not_abool(expr: ResolvedExpr<'db>, ty: Ty<'db>) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::LhsIsNotABool { ty },
        }
    }

    pub fn invalid_enum_variant(
        expr: ResolvedExpr<'db>,
        enum_ty: Ty<'db>,
        variant: SpanIdent<'db>,
    ) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::InvalidEnumVariant { enum_ty, variant },
        }
    }

    pub fn subrange_value_out_of_bounds(
        expr: ResolvedExpr<'db>,
        subrange_ty: Ty<'db>,
        min: u64,
        max: u64,
        value: u64,
    ) -> Self {
        Self {
            expr,
            kind: ExprMismatchKind::SubRangeValueOutOfBounds { expr, subrange_ty, min, max, value },
        }
    }
}

impl<'db> DiagnosticDescription<'db> for ExprMismatch<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match &self.kind {
            ExprMismatchKind::Literal { literal, ty } => match literal {
                LiteralErrorKind::Invalid_BOOL_Literal => {
                    "invalid BOOL literal".to_string()
                }
                LiteralErrorKind::Invalid_UNSIGNED_8_BITS_Literal => {
                    "invalid USINT literal".to_string()
                }
                LiteralErrorKind::Invalid_UNSIGNED_16_BITS_Literal => {
                    "invalid UINT literal".to_string()
                }
                LiteralErrorKind::Invalid_UNSIGNED_32_BITS_Literal => {
                    "invalid UDINT literal".to_string()
                }
                LiteralErrorKind::Invalid_UNSIGNED_64_BITS_Literal => {
                    "invalid ULINT literal".to_string()
                }
                LiteralErrorKind::Invalid_SIGNED_8_BITS_Literal => {
                    "invalid SINT literal".to_string()
                }
                LiteralErrorKind::Invalid_SIGNED_16_BITS_Literal => {
                    "invalid INT literal".to_string()
                }
                LiteralErrorKind::Invalid_SIGNED_32_BITS_Literal => {
                    "invalid DINT literal".to_string()
                }
                LiteralErrorKind::Invalid_SIGNED_64_BITS_Literal => {
                    "invalid LINT literal".to_string()
                }
                LiteralErrorKind::Invalid_REAL_Literal => {
                    "invalid REAL literal".to_string()
                }
                LiteralErrorKind::Invalid_LREAL_Literal => {
                    "invalid LREAL literal".to_string()
                }
                LiteralErrorKind::Invalid_TIME_Literal => {
                    "invalid TIME literal".to_string()
                }
                LiteralErrorKind::Invalid_LTIME_Literal => {
                    "invalid LTIME literal".to_string()
                }
                LiteralErrorKind::Invalid_TOD_Literal => {
                    "invalid TOD literal".to_string()
                }
                LiteralErrorKind::Invalid_LTOD_Literal => {
                    "invalid LTOD literal".to_string()
                }
                LiteralErrorKind::Invalid_DATE_Literal => {
                    "invalid DATE literal".to_string()
                }
                LiteralErrorKind::Invalid_LDATE_Literal => {
                    "invalid LDATE literal".to_string()
                }
                LiteralErrorKind::Invalid_DT_Literal => {
                    "invalid DT literal".to_string()
                }
                LiteralErrorKind::Invalid_LDT_Literal => {
                    "invalid LDT literal".to_string()
                }
                LiteralErrorKind::Invalid_STRING_Literal => {
                    "invalid DSTRINGT literal".to_string()
                }
                LiteralErrorKind::Invalid_WSTRING_Literal => {
                    "invalid WSTRING literal".to_string()
                }
                LiteralErrorKind::TypeMismatch(err) => err.to_owned(),
                LiteralErrorKind::DurationOverflow => "duration overflow".to_string(),
                LiteralErrorKind::ExpectedNumber => "expected a number".to_string(),
                LiteralErrorKind::InvalidNumber(err) => format!("invalid number: {err}"),
                LiteralErrorKind::Invalid_TIME_Components => {
                    "invalid TIME components".to_string()
                }
                LiteralErrorKind::Invalid_TIME_Unit(err) => {
                    format!("invalid TIME unit: {err}")
                }
                LiteralErrorKind::Invalid_DATE_Format(err) => {
                    format!("invalid DATE format: {err}")
                }
                LiteralErrorKind::Invalid_LDATE_Format(err) => {
                    format!("invalid DATE format: {err}")
                }
                LiteralErrorKind::Invalid_TOD_Format(err) => {
                    format!("invalid TOD format: {err}")
                }
                LiteralErrorKind::Invalid_LTOD_Format(err) => {
                    format!("invalid LTOD format: {err}")
                }
                LiteralErrorKind::Invalid_DT_Format(err) => {
                    format!("invalid DT format: {err}")
                }
                LiteralErrorKind::Invalid_LDT_Format(err) => {
                    format!("invalid LDT format: {err}")
                }
                LiteralErrorKind::Incomplete_STRING_XX_Escape => {
                    "incomplete escape sequence in STRING literal".to_string()
                }
                LiteralErrorKind::Invalid_STRING_Hex_Escape => {
                    "invalid hex escape sequence in STRING literal".to_string()
                }
                LiteralErrorKind::Invalid_STRING_CHAR(err) => {
                    format!("invalid escape sequence in STRING literal: {err}")
                }
                LiteralErrorKind::Invalid_WSTRING_Hex_Escape(err) => {
                    format!("invalid escape sequence in STRING literal: {err}")
                }
                LiteralErrorKind::Invalid_WSTRING_Unicode_Scalar(err) => {
                    format!("invalid escape sequence in STRING literal: {err}")
                }
                LiteralErrorKind::Incomplete_WSTRING_XXXX_Escape => {
                    "invalid escape sequence in STRING literal".to_string()
                }
            },
            ExprMismatchKind::TypeMismatch { err } => {
                format!(
                    "type mismatch: expected {}, found {}",
                    err.ty1.type_name(db),
                    err.ty2.type_name(db)
                )
            }
            ExprMismatchKind::ExprTypeMismatch { ty } => {
                format!("type mismatch: expected {}, got {}", ty.type_name(db), self.expr.expr(db).to_string(db))
            }
            ExprMismatchKind::VoidRhs { ty } => {
                "right-hand side is void".to_string()
            }
            ExprMismatchKind::UnresolvedPathError { err } => err.description(db),
            ExprMismatchKind::UnresolvedVarError { err } => err.description(db),
            ExprMismatchKind::LhsIsNotABool { ty } => {
                "left-hand side is not a boolean".to_string()
            }
            ExprMismatchKind::InvalidEnumVariant { enum_ty, variant } => {
                format!(
                    "ENUM '{}' has no variant named '{}'",
                    enum_ty.decl(db).name(db).text(db),
                    variant.text(db)
                )
            }
            ExprMismatchKind::SubRangeValueOutOfBounds { expr, subrange_ty, min, max, value } => {
                format!(
                    "value {} is out of bounds for SUBRANGE {} (expected between {} and {})",
                    value,
                    subrange_ty.decl(db).name(db).text(db),
                    min,
                    max
                )
            }
        }
    }

    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match &self.kind {
            ExprMismatchKind::TypeMismatch { err } => {
                get_decl_and_def_for_ty(db, err.ty1, diag);
                get_decl_and_def_for_ty(db, err.ty2, diag);
            }
            ExprMismatchKind::ExprTypeMismatch { ty } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            ExprMismatchKind::Literal { ty, literal } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            ExprMismatchKind::VoidRhs { ty } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            ExprMismatchKind::LhsIsNotABool { ty } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            ExprMismatchKind::InvalidEnumVariant { enum_ty, variant } => {
                get_decl_and_def_for_ty(db, *enum_ty, diag);
            }
            ExprMismatchKind::SubRangeValueOutOfBounds { expr, subrange_ty, min, max, value } => {
                get_decl_and_def_for_ty(db, *subrange_ty, diag);
            }
            _ => {}
        }
    }
}

impl<'db> DiagnosticDescription<'db> for TypeMismatch<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        format!(
            "type mismatch: expected {}, found {}",
            self.ty1.type_name(db),
            self.ty2.type_name(db)
        )
    }

    fn related(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        get_decl_and_def_for_ty(db, self.ty1, diag);
        get_decl_and_def_for_ty(db, self.ty2, diag);
    }
}
