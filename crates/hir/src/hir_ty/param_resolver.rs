use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::expressions::expression::{ParamAssign, ParamAssignKind},
    hir_ty::{
        expr_resolver::resolve_expr,
        func_call_resolver::{ResolvedParam, ResolvedParamKind},
        ty::Ty,
        ty_var_access_resolver::{
            ResolvedVarKind, ResolvedVarOrigin, ResolvedVarResult, resolve_var_access,
        },
    },
};

pub fn resolve_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: Ty<'db>,
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
                        resolved_param: callee.variables(db).and_then(|signature| {
                            // Try to get the param by index
                            let param = signature.values().nth(formal_index);
                            formal_index += 1;
                            param.map(|p| {
                                ResolvedVarResult::new(
                                    db,
                                    ResolvedVarOrigin::NonFormal(value),
                                    ResolvedVarKind::Param(*p),
                                )
                            })
                        }),
                        value: *resolve_expr(db, value),
                    },
                )
            }
            ParamAssignKind::FormalInput { param, value } => ResolvedParam::new(
                db,
                *param_assign,
                ResolvedParamKind::FormalInput {
                    param,
                    resolved_param: callee.variables(db).and_then(|signature| {
                        signature
                            .get(&param.ident)
                            .or_else(|| {
                                signature
                                    .get(&param.ident)
                                    .filter(|v| v.is_variable_input(db) || v.is_variable_inout(db))
                            })
                            .map(|p| {
                                ResolvedVarResult::new(
                                    db,
                                    ResolvedVarOrigin::Formal(param),
                                    ResolvedVarKind::Param(*p),
                                )
                            })
                    }),
                    value: *resolve_expr(db, value),
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
                    resolved_param: callee.variables(db).and_then(|signature| {
                        signature
                            .get(&param.ident)
                            .filter(|v| v.is_variable_output(db))
                            .map(|p| {
                                ResolvedVarResult::new(
                                    db,
                                    ResolvedVarOrigin::Formal(param),
                                    ResolvedVarKind::Param(*p),
                                )
                            })
                    }),
                    variable: resolve_var_access(db, variable),
                },
            ),
        })
        .collect()
}
