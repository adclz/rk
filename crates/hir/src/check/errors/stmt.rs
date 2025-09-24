use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{diag, IdeDiagnostic, Related};

use crate::{
    check::errors::{
        sem_errors::{AnalysisError, ToIdeDiagnostic},
        utils::{get_decl_and_def_for_ty, get_decl_for_ty},
    }, hir_def::{expressions::statement::Stmt, interned::identifier::SpanIdent}, hir_ty::{
        expr_resolver::ResolvedExpr, stmt_resolver::ResolvedStmt, ty::Ty, ty_path_expr_resolver::ResolvedPathResult, ty_var_access_resolver::ResolvedVarResult
    }, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtError<'db> {
    ContinueOutsideLoop {
        continue_stmt: Stmt<'db>,
    },
    ExitOutsideLoop {
        exit_stmt: Stmt<'db>,
    },
    AssignementToDirectType {
        ty: Ty<'db>,
        var: ResolvedVarResult<'db>,
    },
    AssignementToCallableType {
        ty: Ty<'db>,
        var: ResolvedVarResult<'db>,
    },
    VoidAssignmentRHS {
        ty: Ty<'db>,
        target: ResolvedPathResult<'db>,
    },
    AssignmentToInputVar {
        ty: Ty<'db>,
        var: ResolvedVarResult<'db>,
    },
    AssignBoolExpressionToNonBool {
        ty: Ty<'db>,
        expr: ResolvedExpr<'db>,
    },
    CallANonCallableType {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
    },
    UnusedReturnType {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
        ret: Ty<'db>,
    },
    TooManyParameters {
        stmt: ResolvedStmt<'db>,
        target: Ty<'db>,
        expected: usize,
        found: usize,
    },
    DuplicateParameter {
        param1: SpanIdent<'db>,
        param2: SpanIdent<'db>,
    },
    UnknownNonFormalParam {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
    },
    UnknownFormalInputParam {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
        param: SpanIdent<'db>,
    },
    UnknownFormalOutputParam {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
        param: SpanIdent<'db>,
    },
    MixedFormalNonFormalParams {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
    },
}

impl<'db> From<StmtError<'db>> for AnalysisError<'db> {
    fn from(err: StmtError<'db>) -> Self {
        AnalysisError::StmtError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for StmtError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::AssignementToCallableType { var, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a callable type and can not be assigned",
                        ty.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);

                diag
            }
            Self::AssignementToDirectType { var, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a type and can not be assigned",
                        ty.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);
                diag.with_note(
                    "types can only be assigned if they are declared in a VAR_* section".into(),
                );
                diag
            }
            Self::AssignmentToInputVar { var, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is an input variable and should not be assigned",
                        ty.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::WARNING)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_for_ty(db, *ty, &mut diag);

                diag
            }
            Self::VoidAssignmentRHS { ty, target } => {
                let mut diag = diag()
                    .message("target is of type void".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(target.get_span(db).clone())
                    .call();

                get_decl_for_ty(db, *ty, &mut diag);

                diag
            }
            Self::AssignBoolExpressionToNonBool { ty, expr } => {
                let mut diag = diag()
                    .message(format!(
                        "a boolean expression can not be assigned because '{}' is not a boolean",
                        ty.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);

                diag
            }
            Self::CallANonCallableType { ty, var } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot call non-callable type '{}'",
                        ty.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);

                diag.with_note("only functions, function blocks or methods can be called".into());

                diag
            }
            Self::UnusedReturnType { ty, var, ret } => {
                let mut diag = diag()
                    .message(format!(
                        "unused return type of '{}'",
                        ty.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::WARNING)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ret, &mut diag);
                diag
            }
            Self::TooManyParameters { stmt, target, expected, found } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' expected {expected} parameters, but got {found}",
                        target.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(stmt.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *target, &mut diag);
                diag
            }
            Self::DuplicateParameter { param1, param2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate parameter '{}'",
                        param1.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param1.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "parameter '{}' is already defined here",
                        param2.text(db)
                    ),
                    param2.get_scope_id(db).file(db),
                    param2.get_span(db).clone(),
                ));

                diag
            }
            Self::UnknownNonFormalParam { ty, var } => {
                let mut diag = diag()
                    .message(format!(
                        "unknown non-formal parameter in call to '{}'",
                        ty.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);
                diag
            }
            Self::UnknownFormalInputParam { ty, var, param } => {
                let mut diag = diag()
                    .message(format!("unknown input parameter '{}'", param.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);
                diag
            }
            Self::UnknownFormalOutputParam { ty, var, param } => {
                let mut diag = diag()
                    .message(format!("unknown output parameter '{}'", param.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);
                diag
            }
            Self::ExitOutsideLoop { exit_stmt } => diag()
                .message("exit statement outside of loop".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(exit_stmt.get_span(db))
                .call(),
            Self::ContinueOutsideLoop { continue_stmt } => diag()
                .message("continue statement outside of loop".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(continue_stmt.get_span(db))
                .call(),
            Self::MixedFormalNonFormalParams { ty, var } => {
                let mut diag = diag()
                    .message(format!(
                        "mixed formal and non-formal parameters in call to '{}'",
                        ty.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                diag.with_note("parameters must be either all formal or all non-formal".into());

                diag
            }
        }
    }
}
