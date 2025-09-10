use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{DiagnosticSeverity, DiagnosticTag},
    tree_sitter,
};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    check::errors::{
        literals::LitCheckError,
        sem_errors::{AnalysisError, ToIdeDiagnostic},
        utils::{get_decl_and_def_for_ty, get_decl_for_ty},
    },
    hir_def::expressions::statement::Stmt,
    hir_ty::{
        expr_resolver::ResolvedExpr, ty::Ty, ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::ResolvedVarResult,
    },
    to_proto::ToProto,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtError<'db> {
    ContinueOutsideLoop {
        continue_stmt: Stmt<'db>,
    },
    ExitOutsideLoop {
        exit_stmt: Stmt<'db>,
    },
    Unreachable {
        start: Span,
        end: Span,
    },
    InvalidAssignment {
        var: ResolvedVarResult<'db>,
        ty: Ty<'db>,
    },
    VoidAssignmentTarget {
        ty: Ty<'db>,
        target: ResolvedPathResult<'db>,
    },
    AssignmentToInput {
        var: ResolvedVarResult<'db>,
        ty: Ty<'db>,
    },
    AssignmentIsNotABool {
        ty: Ty<'db>,
        expr: ResolvedExpr<'db>,
    },
    TypeMismatch {
        expr: ResolvedExpr<'db>,
        ty: Ty<'db>,
        ty2: Ty<'db>,
    },
    RecursiveType {
        ty: Ty<'db>,
    },
    LitCheckError {
        ty: Ty<'db>,
        literal: ResolvedExpr<'db>,
        err: LitCheckError,
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
            Self::InvalidAssignment { var, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a type and can not be assigned",
                        ty.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty, &mut diag);

                diag
            }
            Self::AssignmentToInput { var, ty } => {
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
            Self::VoidAssignmentTarget { ty, target } => {
                let mut diag = diag()
                    .message("target is of void type".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(target.get_span(db).clone())
                    .call();

                get_decl_for_ty(db, *ty, &mut diag);

                diag
            }
            Self::AssignmentIsNotABool { ty, expr } => {
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
            Self::TypeMismatch { expr, ty, ty2 } => {
                let mut diag = diag()
                    .message(format!(
                        "type mismatch: '{}' and '{}'",
                        ty.decl(db).name(db).text(db),
                        ty2.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db).clone())
                    .call();

                get_decl_and_def_for_ty(db, *ty2, &mut diag);

                diag
            }
            Self::Unreachable { start, end } => {
                let range = Span::from(tree_sitter::Range {
                    start_byte: start.start_byte,
                    end_byte: end.end_byte,
                    start_point: start.start_point,
                    end_point: end.end_point,
                });

                diag()
                    .message("unreachable code".into())
                    .severity(DiagnosticSeverity::WARNING)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .range(range.clone())
                    .call()
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
            Self::LitCheckError { ty, literal, err } => match err {
                LitCheckError::TypeMismatch(err) => {
                    let mut diag = diag()
                        .message(err.to_string())
                        .severity(DiagnosticSeverity::ERROR)
                        .range(literal.get_span(db).clone())
                        .call();

                    get_decl_and_def_for_ty(db, *ty, &mut diag);

                    diag
                }
                LitCheckError::InvalidFormat { kind, msg } => diag()
                    .message(format!("invalid format for literal '{kind}': {msg}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(literal.get_span(db).clone())
                    .call(),
                LitCheckError::OutOfRange(err) => diag()
                    .message(err.to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(literal.get_span(db).clone())
                    .call(),
            },
            Self::RecursiveType { ty } => diag()
                .message(format!(
                    "recursive type detected for '{}'",
                    ty.decl(db).name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(ty.decl(db).name_span(db).clone())
                .call(),
        }
    }
}
