use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HirNodeInfo,
    check::errors::{
        coerce::{DiagnosticDescription, ExprMismatch, TypeMismatch},
        analysis_error::{AnalysisError, ToIdeDiagnostic},
        utils::{get_decl_and_def_for_ty, get_decl_for_ty},
    },
    hir_def::interned::identifier::SpanIdent,
    hir_ty::{
        expr_resolver::ResolvedExpr,
        stmt_resolver::ResolvedStmt,
        ty::Ty,
        ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::{ResolvedVarResult, VarResolveError},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtError<'db> {
    // Assignments
    UnresolvedAssignmentTarget {
        var: ResolvedVarResult<'db>,
        err: VarResolveError<'db>,
    },
    AssignementToDirectType {
        ty: Ty<'db>,
        var: ResolvedVarResult<'db>,
    },
    AssignementToCallableType {
        ty: Ty<'db>,
        var: ResolvedVarResult<'db>,
    },
    AssignmentToInputVar {
        ty: Ty<'db>,
        var: ResolvedVarResult<'db>,
    },
    AssignmentTypeMismatch {
        err: ExprMismatch<'db>,
    },
    // Function Calls
    UnresolvedFuncCall {
        stmt: ResolvedStmt<'db>,
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
    MixedFormalNonFormalParams {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
    },
    DuplicateParameter {
        param1: SpanIdent<'db>,
        param2: SpanIdent<'db>,
    },
    UnknownNonFormalParam {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
    },
    UnresolvedNonFormalParam {
        var: ResolvedVarResult<'db>,
        err: VarResolveError<'db>,
    },
    UnknownFormalInputParam {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
        param: SpanIdent<'db>,
    },
    UnresolvedInputParam {
        var: ResolvedVarResult<'db>,
        err: VarResolveError<'db>,
    },
    UnknownFormalOutputParam {
        ty: Ty<'db>,
        var: ResolvedPathResult<'db>,
        param: SpanIdent<'db>,
    },
    UnresolvedOutputParam {
        var: ResolvedVarResult<'db>,
        err: VarResolveError<'db>,
    },
    UnresolvedOutputParamTarget {
        var: ResolvedVarResult<'db>,
        err: VarResolveError<'db>,
    },
    ParameterTypeMismatch {
        param: SpanIdent<'db>,
        var: ResolvedVarResult<'db>,
        err: TypeMismatch<'db>,
    },
    ParameterExprMismatch {
        var: ResolvedVarResult<'db>,
        expr: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
    // For and While loops
    UnresolvedControlVar {
        control: ResolvedVarResult<'db>,
        err: VarResolveError<'db>,
    },
    ForLoopStartTypeMismatch {
        start: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
    ForLoopEndTypeMismatch {
        end: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
    ForLoopStepTypeMismatch {
        step: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
    WhileConditionIsNotABool {
        condition: ResolvedExpr<'db>,
    },
    RepeatConditionIsNotABool {
        condition: ResolvedExpr<'db>,
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

                diag.with_note("only functions with return types can be assigned".into());

                diag
            }
            Self::AssignmentTypeMismatch { err } => {
                let mut diag = diag()
                    .message(format!("invalid assignment: {}", err.description(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(err.expr.get_span(db).clone())
                    .call();

                err.note(db, &mut diag);
                err.related(db, &mut diag);
                diag
            }
            Self::UnresolvedAssignmentTarget { var, err } => {
                let mut diag = diag()
                    .message(format!("invalid assignment: {}", err.description(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                err.note(db, &mut diag);
                err.related(db, &mut diag);
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
            Self::UnresolvedFuncCall { stmt } => {
                let mut diag = diag()
                    .message("unresolved function call".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(stmt.get_span(db).clone())
                    .call();
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
            Self::TooManyParameters {
                stmt,
                target,
                expected,
                found,
            } => {
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
                    .message(format!("duplicate parameter '{}'", param1.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param1.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!("parameter '{}' is already defined here", param2.text(db)),
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
            Self::UnresolvedInputParam { var, err } => {
                let mut diag = diag()
                    .message(format!(
                        "unresolved input parameter: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();
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
            Self::UnresolvedOutputParam { var, err } => {
                let mut diag = diag()
                    .message(format!(
                        "unresolved output parameter: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();
                diag
            }
            Self::UnresolvedOutputParamTarget { var, err } => {
                let mut diag = diag()
                    .message(format!(
                        "unresolved output parameter target: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();
                diag
            }
            Self::UnresolvedNonFormalParam { var, err } => {
                let mut diag = diag()
                    .message(format!(
                        "unresolved non-formal parameter: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();
                diag
            }
            Self::ParameterTypeMismatch { var, param, err } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid output assignment: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);
                diag
            }
            Self::ParameterExprMismatch { var, expr, err } => {
                let mut diag = diag()
                    .message(format!(
                        "parameter expression mismatch: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.get_span(db).clone())
                    .call();

                err.note(db, &mut diag);
                err.related(db, &mut diag);
                diag
            }
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
            Self::UnresolvedControlVar { control, err } => {
                let mut diag = diag()
                    .message(format!(
                        "unresolved control variable: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(control.get_span(db).clone())
                    .call();
                diag
            }
            Self::ForLoopStartTypeMismatch { start, err } => {
                let mut diag = diag()
                    .message(format!("invalid FOR loop start: {}", err.description(db)))
                    .range(start.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();
                diag
            }
            Self::ForLoopEndTypeMismatch { end, err } => {
                let mut diag = diag()
                    .message(format!("invalid FOR loop end: {}", err.description(db)))
                    .range(end.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();

                diag
            }
            Self::ForLoopStepTypeMismatch { step, err } => {
                let mut diag = diag()
                    .message(format!("invalid FOR loop step: {}", err.description(db)))
                    .range(step.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();

                diag
            }
            Self::WhileConditionIsNotABool { condition } => {
                let mut diag = diag()
                    .message("WHILE condition is not returning a boolean".into())
                    .range(condition.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();
                diag
            }
            Self::RepeatConditionIsNotABool { condition } => {
                let mut diag = diag()
                    .message("REPEAT condition is not returning a boolean".into())
                    .range(condition.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();
                diag
            }
        }
    }
}
