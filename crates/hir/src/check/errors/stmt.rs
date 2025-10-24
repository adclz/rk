use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    check::errors::{
            analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
            coerce::{ExprMismatch, TypeMismatch},
            path_error::AccessError,
        }, hir_def::{expressions::expression::PathExpr, interned::identifier::SpanIdent}, hir_ty::{expr_resolver::ResolvedExpr, ty_var_access_resolver::ResolvedAccess}, query_string::{fuzzy_method::fuzzy_method_parameters, fuzzy_pou::fuzzy_pou_items}, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtError<'db> {
    EmptyPathExpression {
        var: ResolvedAccess<'db>,
    },
    // Assignments
    UnresolvedAssignmentTarget {
        var: ResolvedAccess<'db>,
        err: AccessError<'db>,
    },
    InvalidAssignmentTarget {
        var: ResolvedAccess<'db>,
    },
    AssignmentToDirectType {
        var: ResolvedAccess<'db>,
    },
    AssignmentToCallableType {
        var: ResolvedAccess<'db>,
    },
    AssignmentToInputVar {
        var: ResolvedAccess<'db>,
    },
    AssignmentTypeMismatch {
        err: ExprMismatch<'db>,
    },
    // Function Calls
    UnresolvedFuncCall {
        call: ResolvedAccess<'db>,
    },
    CallANonCallableType {
        call: ResolvedAccess<'db>,
    },
    CallADirectType {
        call: ResolvedAccess<'db>,
    },
    UnusedReturnType {
        call: ResolvedAccess<'db>,
    },
    TooManyParameters {
        call: ResolvedAccess<'db>,
        expected: usize,
        found: usize,
    },
    MixedFormalNonFormalParams {
        call: ResolvedAccess<'db>,
    },
    DuplicateParameter {
        param1: SpanIdent<'db>,
        param2: SpanIdent<'db>,
    },
    UnknownNonFormalParam {
        call: ResolvedAccess<'db>,
    },
    UnresolvedNonFormalParam {
        var: ResolvedAccess<'db>,
        err: AccessError<'db>,
    },
    UnknownFormalInputParam {
        call: ResolvedAccess<'db>,
        param: SpanIdent<'db>,
    },
    UnresolvedInputParam {
        var: ResolvedAccess<'db>,
        err: AccessError<'db>,
    },
    UnknownFormalOutputParam {
        call: ResolvedAccess<'db>,
        param: SpanIdent<'db>,
    },
    UnresolvedOutputParam {
        var: ResolvedAccess<'db>,
        err: AccessError<'db>,
    },
    UnresolvedOutputParamTarget {
        var: ResolvedAccess<'db>,
        err: AccessError<'db>,
    },
    ParameterTypeMismatch {
        param: SpanIdent<'db>,
        var: ResolvedAccess<'db>,
        err: TypeMismatch<'db>,
    },
    ParameterExprMismatch {
        var: ResolvedAccess<'db>,
        expr: ResolvedExpr<'db>,
        err: ExprMismatch<'db>,
    },
    // For and While loops
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
            Self::EmptyPathExpression { var } => {
                diag()
                    .message("unused path, you might want to do something with it".into())
                    .severity(DiagnosticSeverity::WARNING)
                    .range(var.get_span(db).clone())
                    .call()
            },
            Self::AssignmentToCallableType { var } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a callable type and can not be assigned",
                        var.decl_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

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

            Self::InvalidAssignmentTarget { var } => diag()
                .message(format!("'{}' can not be mutated", var.decl_name(db),))
                .severity(DiagnosticSeverity::ERROR)
                .range(var.get_span(db).clone())
                .call(),

            Self::AssignmentToDirectType { var } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a type and can not be assigned",
                        var.decl_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                diag.with_note(
                    "types can only be assigned if they are declared in a VAR_* section".into(),
                );
                diag
            }
            Self::AssignmentToInputVar { var } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is an input variable and can not be assigned",
                        var.decl_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var.get_span(db).clone())
                    .call();

                let _ = var.resolved(db).map(|p| {
                    p.diag_with_location(db, &mut diag);
                });
                diag
            }
            Self::UnresolvedFuncCall { call } => diag()
                .message("unresolved function call".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(call.get_span(db).clone())
                .call(),
            Self::CallANonCallableType { call } => {
                let mut diag: IdeDiagnostic = diag()
                    .message(format!(
                        "cannot call non-callable type '{}'",
                        call.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call.get_span(db).clone())
                    .call();

                diag.with_note("only functions, function blocks or methods can be called".into());

                diag
            }
            Self::CallADirectType { call } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a direct type and can not be called",
                        call.decl_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call.get_span(db).clone())
                    .call();

                diag.with_note("only FUNCTIONS and METHODS or body from declared CLASS/FUNCTIOn_BLOCKS can called".into());

                diag
            }
            Self::UnusedReturnType { call } => {
                

                diag()
                    .message(format!("unused return type of '{}'", call.decl_name(db)))
                    .severity(DiagnosticSeverity::WARNING)
                    .range(call.get_span(db).clone())
                    .call()
            }
            Self::TooManyParameters {
                call,
                expected,
                found,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' expected {expected} parameters, but got {found}",
                        call.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call.get_span(db).clone())
                    .call();

                let _ = call.resolved(db).map(|p| {
                    p.diag_with_location(db, &mut diag);
                });
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
            Self::UnknownNonFormalParam { call } => {
                

                diag()
                    .message(format!(
                        "unknown non-formal parameter in call to '{}'",
                        call.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call.get_span(db).clone())
                    .call()
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

                err.related(db, &mut diag);
                err.note(db, &mut diag);
                diag
            }
            Self::UnknownFormalInputParam { call, param } => {
                let mut diag = diag()
                    .message(format!("unknown input parameter '{}'", param.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.get_span(db).clone())
                    .call();

                if let Some(m) = call.as_pou(db) {
                    fuzzy_pou_items(db, m, &mut diag, param.ident.text(db).as_str());
                }
                diag
            }
            Self::UnknownFormalOutputParam { call, param } => {
                let mut diag = diag()
                    .message(format!("unknown output parameter '{}'", param.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.get_span(db).clone())
                    .call();

                if let Some(m) = call.as_pou(db) {
                    fuzzy_pou_items(db, m, &mut diag, param.ident.text(db).as_str());
                }
                diag
            }
            Self::UnresolvedOutputParam { var, err } => diag()
                .message(format!(
                    "unresolved output parameter: {}",
                    err.description(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(var.get_span(db).clone())
                .call(),
            Self::UnresolvedOutputParamTarget { var, err } => diag()
                .message(format!(
                    "unresolved output parameter target: {}",
                    err.description(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(var.get_span(db).clone())
                .call(),
            Self::UnresolvedNonFormalParam { var, err } => diag()
                .message(format!(
                    "unresolved non-formal parameter: {}",
                    err.description(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(var.get_span(db).clone())
                .call(),
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
            Self::MixedFormalNonFormalParams { call } => {
                let mut diag = diag()
                    .message(format!(
                        "mixed formal and non-formal parameters in call to '{}'",
                        call.decl_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(call.get_span(db).clone())
                    .call();

                let _ = call.resolved(db).map(|p| {
                    p.diag_with_location(db, &mut diag);
                });
                diag.with_note("parameters must be either all formal or all non-formal".into());

                diag
            }
            Self::ForLoopStartTypeMismatch { start, err } => {
                let mut diag = diag()
                    .message(format!("invalid FOR loop start: {}", err.description(db)))
                    .range(start.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);
                diag
            }
            Self::ForLoopEndTypeMismatch { end, err } => {
                let mut diag = diag()
                    .message(format!("invalid FOR loop end: {}", err.description(db)))
                    .range(end.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);
                diag
            }
            Self::ForLoopStepTypeMismatch { step, err } => {
                let mut diag = diag()
                    .message(format!("invalid FOR loop step: {}", err.description(db)))
                    .range(step.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);
                diag
            }
            Self::WhileConditionIsNotABool { condition } => diag()
                .message("WHILE condition is not returning a boolean".into())
                .range(condition.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .call(),
            Self::RepeatConditionIsNotABool { condition } => diag()
                .message("REPEAT condition is not returning a boolean".into())
                .range(condition.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .call(),
        }
    }
}
