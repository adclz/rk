use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{expressions::{expression::{Expr, FuncCall, ParamAssign, ParamAssignKind}, invocation::Invocation}, interned::identifier::SpanIdent, pous::{pou::PouDecl, variable::VariableDecl}},
    hir_ty::{
        expr_resolver::resolve_expr, func_call_resolver::{ResolvedParam, ResolvedParamKind}, inheritance_solver::MethodRef, signatures::LocalVariables, ty_var_access_resolver::{resolve_var_access, CallSite, ResolvedAccess}, walk::{Adjustement, ResolvedPath, ResolvedPathKind, ResolvedPathResult}
    },
};

#[salsa::tracked]
pub fn resolve_func_call_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: PouDecl<'db>,
    caller: FuncCall<'db>,
) -> Vec<ResolvedParam<'db>> {
    resolve_parameters(db, callee, &caller.params(db))
}

#[salsa::tracked]
pub fn resolve_method_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: MethodRef<'db>,
    caller: FuncCall<'db>,
) -> Vec<ResolvedParam<'db>> {
    resolve_parameters(db, callee, &caller.params(db))
}

#[salsa::tracked]
pub fn resolve_invocation_method_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: MethodRef<'db>,
    caller: Invocation<'db>,
) -> Vec<ResolvedParam<'db>> {
    resolve_parameters(db, callee, &caller.params(db))
}

#[salsa::tracked]
pub fn resolve_invocation_func_call_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: PouDecl<'db>,
    caller: Invocation<'db>,
) -> Vec<ResolvedParam<'db>> {
    resolve_parameters(db, callee, &caller.params(db))
}

fn resolve_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: impl LocalVariables<'db>,
    caller: &[ParamAssign<'db>],
) -> Vec<ResolvedParam<'db>> {
    let mut formal_index = 0;

    caller
        .iter()
        .map(|param_assign| match param_assign.kind(db) {
            ParamAssignKind::NonFormal { value } => {
                ResolvedParam::new(
                    db,
                    *param_assign,
                    ResolvedParamKind::NonFormal {
                        resolved_param: {
                            // Try to get the param by index
                            let param = callee.local_variables(db).values().nth(formal_index);
                            formal_index += 1;
                            param.map(|p| {
                                var_into_non_formal(db, *p, value)
                            })
                        },
                        value,
                    },
                )
            }
            ParamAssignKind::FormalInput { param, value } => ResolvedParam::new(
                db,
                *param_assign,
                ResolvedParamKind::FormalInput {
                    param,
                    resolved_param: {
                        callee
                            .local_variables(db)
                            .get(&param.ident)
                            .filter(|v| v.is_input(db) || v.is_in_out(db))
                            .map(|p| {
                                var_into_formal(db, *p, param)
                            })
                    },
                    value,
                },
            ),
            ParamAssignKind::FormalOutput {
                not,
                param,
                variable,
            } => ResolvedParam::new(
                db,
                *param_assign,
                ResolvedParamKind::FormalOutput {
                    not,
                    param,
                    resolved_param: {
                        callee
                            .local_variables(db)
                            .get(&param.ident)
                            .filter(|v| v.is_output(db))
                            .map(|p| {
                                var_into_formal(db, *p, param)
                            })
                    },
                    variable,
                },
            ),
        })
        .collect()
}

#[salsa::tracked]
pub fn var_into_non_formal<'db>(
    db: &'db dyn BaseDatabase,
    var: VariableDecl<'db>,
    expr: Expr<'db>,
) -> ResolvedAccess<'db> {
    ResolvedAccess::new(
        db,
        ResolvedPathResult::Ok(ResolvedPath {
            kind: ResolvedPathKind::Variable(var),
            expr: CallSite::NonFormal(expr),
            adjustement: Adjustement::None,
        }),
        vec![],
    )
}

#[salsa::tracked]
pub fn var_into_formal<'db>(
    db: &'db dyn BaseDatabase,
    var: VariableDecl<'db>,
    ident: SpanIdent<'db>,
) -> ResolvedAccess<'db> {
    ResolvedAccess::new(
        db,
        ResolvedPathResult::Ok(ResolvedPath {
            kind: ResolvedPathKind::Variable(var),
            expr: CallSite::Formal(ident),
            adjustement: Adjustement::None,
        }),
        vec![],
    )
}
