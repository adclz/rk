use std::{error::Error, fmt::Display, sync::Arc};

use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    check::errors::{
        sem_errors::{AnalysisError, ToIdeDiagnostic},
        stmt::StmtError,
        utils::get_decl_and_def_for_ty,
    }, hir_def::{expressions::expression::Expr, interned::identifier::SpanIdent}, hir_ty::{expr_resolver::ResolvedExpr, ty::Ty}, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum CoerceError<'db> {
    UnknownTypeExpr {
        expr: ResolvedExpr<'db>,
        ty: Ty<'db>,
    },
    ExprTypeMismatch {
        expr: ResolvedExpr<'db>,
        ty1: Ty<'db>,
        ty2: Ty<'db>,
    },
    ParamTypeMismatch {
        param: SpanIdent<'db>,
        ty1: Ty<'db>,
        ty2: Ty<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct TypeMismatch<'db> {
    pub ty1: Ty<'db>,
    pub ty2: Ty<'db>,
}

impl<'db> CoerceError<'db> {
    pub fn new_expr_type_mismatch(expr: ResolvedExpr<'db>, err: TypeMismatch<'db>) -> Self {
        Self::ExprTypeMismatch {
            expr,
            ty1: err.ty1,
            ty2: err.ty2,
        }
    }

    pub fn new_param_type_mismatch(param: SpanIdent<'db>, err: TypeMismatch<'db>) -> Self {
        Self::ParamTypeMismatch {
            param,
            ty1: err.ty1,
            ty2: err.ty2,
        }
    }
}

impl<'db> From<CoerceError<'db>> for AnalysisError<'db> {
    fn from(err: CoerceError<'db>) -> Self {
        AnalysisError::CoerceError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for CoerceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match &self {
            CoerceError::UnknownTypeExpr { expr, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot coerce expression to type '{}'",
                        ty.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);

                diag
            }
            CoerceError::ExprTypeMismatch { expr, ty1, ty2 } => {
                let mut diag = diag()
                    .message(format!(
                        "type mismatch: '{}' and '{}'",
                        ty1.decl(db).name(db).text(db),
                        ty2.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty2, &mut diag);

                diag
            }
            CoerceError::ParamTypeMismatch { param, ty1, ty2 } => {
                let mut diag = diag()
                    .message(format!(
                        "type mismatch for parameter '{}': '{}' and '{}'",
                        param.text(db),
                        ty1.decl(db).name(db).text(db),
                        ty2.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty2, &mut diag);

                diag
            }
        }
    }
}
