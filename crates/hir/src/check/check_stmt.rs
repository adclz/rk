use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::{
        check_semantic_index::Check,
        coerce::{coerce_ty_with_expr, coerce_ty_with_ty},
        errors::{sem_errors::AnalysisError, stmt::StmtError},
    },
    hir_def::expressions::{spec::ElementarySpec, statement::Stmt},
    hir_ty::{
        TyInfo,
        expr_resolver::{ResolvedExpr, ResolvedExprKind},
        stmt_resolver::{ResolvedParam, ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::{Ty, TyKind},
        ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::ResolvedVarResult,
    },
};

impl<'db> Check<'db> for Vec<Stmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        for stmt in self {
            resolve_stmt(db, *stmt).check(db, errors);
        }
    }
}

impl<'db> Check<'db> for Vec<ResolvedStmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        for stmt in self {
            stmt.check(db, errors);
        }
    }
}

impl<'db> Check<'db> for ResolvedStmt<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        match self.kind(db) {
            ResolvedStmtKind::For { body, .. } => {
                body.check(db, errors);
            }
            ResolvedStmtKind::While { body, .. } => {
                body.check(db, errors);
            }
            ResolvedStmtKind::Repeat { body, .. } => {
                body.check(db, errors);
            }
            ResolvedStmtKind::If {
                then,
                else_,
                else_if,
                ..
            } => {
                then.check(db, errors);

                else_.check(db, errors);

                else_if.iter().for_each(|(_, s)| s.check(db, errors));
            }
            ResolvedStmtKind::Assignment { var, target } => {
                if let Err(err) = check_assignment(db, *var, *target) {
                    errors.push(err);
                }
            }
            ResolvedStmtKind::FuncCall { target, params } => {
                if let Err(err) = check_func_call(db, *target, params, errors) {
                    errors.push(err);
                }
            }
            _ => {}
        }
    }
}

fn check_assignment<'db>(
    db: &'db dyn BaseDatabase,
    var: ResolvedVarResult<'db>,
    target: ResolvedExpr<'db>,
) -> Result<(), AnalysisError<'db>> {
    let ty_var = var.ty(db)?;

    // Variables in VAR_INPUT can not be mutated
    if var.is_input(db) {
        return Err(StmtError::AssignmentToInputVar { var, ty: ty_var }.into());
    }

    // Direct type
    if !ty_var.is_variable(db) {
        return Err(StmtError::AssignementToDirectType { var, ty: ty_var }.into());
    }

    // POUs can not be mutated
    if ty_var.is_callable(db) {
        return Err(StmtError::AssignementToCallableType { var, ty: ty_var }.into());
    }

    coerce_ty_with_expr(db, ty_var, target)
}

fn check_func_call<'db>(
    db: &'db dyn BaseDatabase,
    target: ResolvedPathResult<'db>,
    params: &Vec<ResolvedParam<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let ty_target = target.ty(db)?;
    if !ty_target.is_callable(db) {
        return Err(StmtError::CallANonCallableType {
            ty: ty_target,
            var: target,
        }
        .into());
    }

    if let Some(ret) = ty_target.has_return_type(db) {
        errors.push(
            StmtError::UnusedReturnType {
                ty: ty_target,
                var: target,
                ret: ret,
            }
            .into(),
        )
    }

    let signature = ty_target.to_signature(db).unwrap();
    let mut with_param_name = false;

    params.iter().for_each(|p| match p {
        ResolvedParam::Input { param, value } => {
            if let Some(name) = param {
                if let Some(other_p) = signature.inputs.get(name) {
                    if let Err(err) = coerce_ty_with_expr(db, *other_p, *value) {
                        errors.push(err);
                    }
                } else {
                    errors.push(
                        StmtError::UnknownInputParam {
                            ty: ty_target,
                            var: target,
                            param: *name,
                        }
                        .into(),
                    )
                }
            }
        }
        ResolvedParam::Output {
            not,
            param,
            variable,
        } => {
            if let Some(other_p) = signature.outputs.get(param) {
            } else {
                errors.push(
                    StmtError::UnknownOutputParam {
                        ty: ty_target,
                        var: target,
                        param: *param,
                    }
                    .into(),
                )
            }
        }
    });

    Ok(())
}
