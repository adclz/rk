use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::check::errors::e1_duplicates::DuplicateError;
use crate::check::errors::e3_type::TypeError;
use crate::check::errors::e10_control_flow::ControlFlowError;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::pous::variable::VariableDecl;
use crate::{
    CallSite,
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::expressions::expression::{FuncCall, ParamAssignKind},
    hir_ty::{body::BodyInferenceResult, infer::expr::InferExprCtx, resolver::Resolver, ty::Type},
};

pub fn resolve_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    resolver.resolve_begin_path_expr(db, func_call.path(db), None, ctx);

    let access_typ = ctx.get_type_of_begin_path_expr(db, func_call.path(db));

    let target_typ = ctx
        .type_of_begin_expr_with_adjustments(db, func_call.path(db))
        .normalize(db);

    if access_typ.is_never() || target_typ.is_never() {
        return;
    }

    // FUNCTION_BLOCKs can only be called if they are variables
    if !access_typ.is_variable() && target_typ.is_fb() {
        ctx.errors.push(
            ControlFlowError::CallNonCallableType {
                typ: target_typ,
                func_call,
            }
            .to_diagnostic(db),
        );
        return;
    }

    let callable = match target_typ.as_callable(db) {
        Some(callable) => callable,
        None => {
            ctx.errors.push(
                ControlFlowError::CallNonCallableType {
                    typ: target_typ,
                    func_call,
                }
                .to_diagnostic(db),
            );
            return;
        }
    };

    // func call requires the type to be a [`CallableType`] otherwise the coercion layer will
    // assume we are calling a non-callable type
    if let Some(expr) = func_call.path(db).expr(db) {
        ctx.type_of_path_expr
            .insert(expr, Type::CallableType(callable));
    }

    let mut seen = FxHashMap::default();
    let mut formal_idx = 0;
    let len = func_call.params(db).len();

    if len > callable.var_len_params(db) {
        ctx.errors.push(
            ResolveError::IncorrectNumberOfParameters {
                expected: callable.var_len_params(db),
                actual: len,
                func_call,
                callable,
            }
            .to_diagnostic(db),
        );
    }

    for parameter in func_call.params(db) {
        match parameter.kind(db) {
            ParamAssignKind::NonFormal { value } => {
                // Try to get the param by index
                let var = callable
                    .def_map(db)
                    .local_variables
                    .values()
                    .nth(formal_idx);

                if let Some(var) = var {
                    coerce_with_var_target(db, resolver, value, *var, ctx);

                    if var.is_output(db) {
                        ctx.errors.push(
                            ResolveError::OutputParameterUsedAsInput {
                                func: callable,
                                var: *var,
                                expr: value,
                                param: formal_idx,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    ctx.variable_of_param.insert(parameter, *var);
                } else {
                    ctx.errors.push(
                        ResolveError::UnknownNonFormalParameter {
                            func: callable,
                            expr: value,
                            param: formal_idx,
                        }
                        .to_diagnostic(db),
                    );
                }
                formal_idx += 1;
            }
            ParamAssignKind::FormalInput { param, value } => {
                if let Some(seen) = seen.insert(param.ident, parameter) {
                    ctx.errors.push(
                        DuplicateError::Parameter {
                            param_1: seen,
                            param_2: parameter,
                            name: param.ident,
                        }
                        .to_diagnostic(db),
                    );
                    continue;
                }
                let var = callable.def_map(db).local_variables.get(&param.ident);

                if let Some(var) = var {
                    coerce_with_var_target(db, resolver, value, *var, ctx);
                    ctx.variable_of_param.insert(parameter, *var);
                } else {
                    ctx.errors.push(
                        ResolveError::UnknownInputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            ParamAssignKind::FormalOutput {
                not,
                param,
                variable,
            } => {
                // check duplicates
                if let Some(seen) = seen.insert(param.ident, parameter) {
                    ctx.errors.push(
                        DuplicateError::Parameter {
                            param_1: seen,
                            param_2: parameter,
                            name: param.ident,
                        }
                        .to_diagnostic(db),
                    );
                    continue;
                }

                if let Some(lhs_var) = callable.def_map(db).local_variables.get(&param.ident) {
                    let lhs_typ = Type::new_var(db, *lhs_var);

                    resolver.resolve_variable_access(db, variable, ctx);
                    let call_site = CallSite::from_scoped(db, &variable);

                    let rhs_typ = ctx.type_of_variable_access_with_adjustments(db, variable);

                    // is the variable assignable?
                    rhs_typ.check_assignable(db, call_site, ctx);

                    // type coercion
                    lhs_typ
                        .coerce_with_type(db, rhs_typ, None, resolver)
                        .map_err(|err| {
                            ctx.errors
                                .push(err.into_non_assignable(db, rhs_typ, call_site))
                        })
                        .ok();

                    ctx.variable_of_param.insert(parameter, *lhs_var);
                } else {
                    ctx.errors.push(
                        ResolveError::UnknownOutputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
        }
    }
}

fn coerce_with_var_target<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    expr: Expr<'db>,
    var: VariableDecl<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let mut caller_infer_ctx = InferExprCtx::new(resolver);
    caller_infer_ctx.resolve_expr(db, expr, ctx);
    caller_infer_ctx.check_expr(db, expr, ctx);

    if let Err(e) = caller_infer_ctx.coerce_var_decl_with_expr(db, var, expr, ctx) {
        ctx.errors.push(
            TypeError::NotAssignable {
                base_target: Type::new_var(db, var),
                lhs: e.expected,
                rhs: e.actual,
                adjustment: e.adjustment,
                expr: CallSite::from_scoped(db, &expr),
            }
            .to_diagnostic(db),
        );
    }
}
