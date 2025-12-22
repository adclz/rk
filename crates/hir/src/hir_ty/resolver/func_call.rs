use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir_ty::infer::coerce::unify_var_access;
use crate::{
    CallSite,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::expressions::expression::{FuncCall, ParamAssignKind},
    hir_ty::{
        body_inference::BodyInferenceResult, infer::expr::InferExprCtx, resolver::Resolver,
        ty::Type,
    },
};

pub fn resolve_func_call<'db>(
    db: &'db dyn BaseDatabase,
    resolver: Resolver<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> Type<'db> {
    let mut typ = resolver.resolve_begin_path_expr(db, func_call.path(db), ctx);

    if let Some(callable) = typ.as_callable(db) {

        // func call requires the type to be a [`CallableType`] otherwise the coercion layer will
        // assume we are calling a non-callable type
        if let Some(expr) = func_call.path(db).expr(db) {
            typ = Type::CallableType(callable);
            ctx.type_of_path_expr.insert(expr, typ);
        }

        let mut seen = FxHashMap::default();
        let mut formal_idx = 0;
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
                        let mut expr_ctx = InferExprCtx::new(resolver);
                        let typ = expr_ctx.infer_expr(db, value, ctx);
                        expr_ctx.inference_table.resolve_completly(db, resolver, ctx);

                        if let Err(e) = Type::new_var(db, *var).coerce_with_type(db, typ, resolver) {
                            ctx.errors.push(
                                TypeError::NotAssignable {
                                    base_target: Type::new_var(db, *var),
                                    target: e.expected,
                                    value: e.actual,
                                    expr: CallSite::from_expr(db, value),
                                }
                                .to_diagnostic(db),
                            );
                        }

                        if var.is_output(db) {
                            ctx.errors.push(
                                BodyInferenceError::OutputParameterUsedAsInput {
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
                            BodyInferenceError::UnknownNonFormalParameter {
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
                            BodyInferenceError::DuplicateParameter {
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
                        ctx.variable_of_param.insert(parameter, *var);
                    } else {
                        ctx.errors.push(
                            BodyInferenceError::UnknownInputParameter {
                                func: callable,
                                param: param,
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
                            BodyInferenceError::DuplicateParameter {
                                param_1: seen,
                                param_2: parameter,
                                name: param.ident,
                            }
                            .to_diagnostic(db),
                        );
                        continue;
                    }

                    if let Some(var) = callable.def_map(db).local_variables.get(&param.ident) {
                        let ty = Type::new_var(db, *var);
                        unify_var_access(db, ty, variable, resolver, ctx);
                        ctx.variable_of_param.insert(parameter, *var);
                        return ty;
                    } else {
                        ctx.errors.push(
                            BodyInferenceError::UnknownOutputParameter {
                                func: callable,
                                param: param,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    return Type::Never;
                }
            }
        }
        return typ;
    } else {
        // not a callable type
        ctx.errors.push(
            BodyInferenceError::CallNonCallableType {
                typ: typ,
                func_call: func_call,
            }
            .to_diagnostic(db),
        );
    }
    Type::Never
}
