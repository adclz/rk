use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        errors::{
            analysis_error::DiagnosticDescription, literals::{LiteralError, LiteralErrorKind}, path_error::PathResolveError, utils::{get_candidates, get_decl_and_def_for_ty}, var_error::VarResolveError
        },
        recovery::pou::fuzzy_pou_local_items,
    }, hir_def::{scope::ScopeKind, semantic_index::semantic_index}, hir_ty::{
        expr_resolver::ResolvedExpr, ty::Ty, ty_path_expr_resolver::ResolvedPathResult,
    }, TypeInfo
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

    pub fn expr_void(
        expr: ResolvedExpr<'db>,
        ty: Ty<'db>,
    ) -> Self {
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
}

impl<'db> DiagnosticDescription<'db> for ExprMismatch<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match &self.kind {
            ExprMismatchKind::Literal { literal, ty } => match literal {
                LiteralErrorKind::Invalid_BOOL_Literal => {
                    format!("invalid BOOL literal")
                }
                LiteralErrorKind::Invalid_UNSIGNED_8_BITS_Literal => {
                    format!("invalid USINT literal")
                }
                LiteralErrorKind::Invalid_UNSIGNED_16_BITS_Literal => {
                    format!("invalid UINT literal")
                }
                LiteralErrorKind::Invalid_UNSIGNED_32_BITS_Literal => {
                    format!("invalid UDINT literal")
                }
                LiteralErrorKind::Invalid_UNSIGNED_64_BITS_Literal => {
                    format!("invalid ULINT literal")
                }
                LiteralErrorKind::Invalid_SIGNED_8_BITS_Literal => {
                    format!("invalid SINT literal")
                }
                LiteralErrorKind::Invalid_SIGNED_16_BITS_Literal => {
                    format!("invalid INT literal")
                }
                LiteralErrorKind::Invalid_SIGNED_32_BITS_Literal => {
                    format!("invalid DINT literal")
                }
                LiteralErrorKind::Invalid_SIGNED_64_BITS_Literal => {
                    format!("invalid LINT literal")
                }
                LiteralErrorKind::Invalid_REAL_Literal => {
                    format!("invalid REAL literal")
                }
                LiteralErrorKind::Invalid_LREAL_Literal => {
                    format!("invalid LREAL literal")
                }
                LiteralErrorKind::Invalid_TIME_Literal => {
                    format!("invalid TIME literal")
                }
                LiteralErrorKind::Invalid_LTIME_Literal => {
                    format!("invalid LTIME literal")
                }
                LiteralErrorKind::Invalid_TOD_Literal => {
                    format!("invalid TOD literal")
                }
                LiteralErrorKind::Invalid_LTOD_Literal => {
                    format!("invalid LTOD literal")
                }
                LiteralErrorKind::Invalid_DATE_Literal => {
                    format!("invalid DATE literal")
                }
                LiteralErrorKind::Invalid_LDATE_Literal => {
                    format!("invalid LDATE literal")
                }
                LiteralErrorKind::Invalid_DT_Literal => {
                    format!("invalid DT literal")
                }
                LiteralErrorKind::Invalid_LDT_Literal => {
                    format!("invalid LDT literal")
                }
                LiteralErrorKind::Invalid_STRING_Literal => {
                    format!("invalid DSTRINGT literal")
                }
                LiteralErrorKind::Invalid_WSTRING_Literal => {
                    format!("invalid WSTRING literal")
                }
                LiteralErrorKind::TypeMismatch(err) => err.to_owned(),
                LiteralErrorKind::DurationOverflow => format!("duration overflow"),
                LiteralErrorKind::ExpectedNumber => format!("expected a number"),
                LiteralErrorKind::InvalidNumber(err) => format!("invalid number: {}", err),
                LiteralErrorKind::Invalid_TIME_Components => {
                    format!("invalid TIME components")
                }
                LiteralErrorKind::Invalid_TIME_Unit(err) => {
                    format!("invalid TIME unit: {}", err)
                }
                LiteralErrorKind::Invalid_DATE_Format(err) => {
                    format!("invalid DATE format: {}", err)
                }
                LiteralErrorKind::Invalid_LDATE_Format(err) => {
                    format!("invalid DATE format: {}", err)
                }
                LiteralErrorKind::Invalid_TOD_Format(err) => {
                    format!("invalid TOD format: {}", err)
                }
                LiteralErrorKind::Invalid_LTOD_Format(err) => {
                    format!("invalid LTOD format: {}", err)
                }
                LiteralErrorKind::Invalid_DT_Format(err) => {
                    format!("invalid DT format: {}", err)
                }
                LiteralErrorKind::Invalid_LDT_Format(err) => {
                    format!("invalid LDT format: {}", err)
                }
                LiteralErrorKind::Incomplete_STRING_XX_Escape => {
                    format!("incomplete escape sequence in STRING literal")
                }
                LiteralErrorKind::Invalid_STRING_Hex_Escape => {
                    format!("invalid hex escape sequence in STRING literal")
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
                    format!("invalid escape sequence in STRING literal")
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
                format!("type mismatch: expected {}", ty.type_name(db))
            }
            ExprMismatchKind::VoidRhs { ty } => {
                format!("right-hand side is void")
            }
            ExprMismatchKind::UnresolvedPathError { err } => err.description(db),
            ExprMismatchKind::UnresolvedVarError { err } => err.description(db),
            ExprMismatchKind::LhsIsNotABool { ty } => {
                format!("left-hand side is not a boolean")
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
