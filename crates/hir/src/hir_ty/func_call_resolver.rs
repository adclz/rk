use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::expression::{FuncCall, ParamAssign, ParamAssignKind},
        interned::identifier::SpanIdent,
        scope::FileScopeId,
    },
    hir_ty::{
        expr_resolver::{ResolvedExpr, resolve_expr},
        ty::Ty,
        ty_path_expr_resolver::{ResolvedPathResult, resolved_path_expr},
        ty_var_access_resolver::{
            ResolvedVarKind, ResolvedVarOrigin, ResolvedVarResult, resolve_var_access,
        },
    },
};

pub struct ResolvedFuncCall<'db> {
    pub target: ResolvedPathResult<'db>,
    pub params: Vec<ResolvedParam<'db>>,
}

impl<'db> FuncCall<'db> {
    pub fn resolve_func_call(&self, db: &'db dyn BaseDatabase) -> ResolvedFuncCall<'db> {
        let target = *resolved_path_expr(db, self.path);
        let target_ty = target.ty(db).ok();
        let mut formal_index = 0;

        ResolvedFuncCall {
            target,
            params: self
                .params
                .iter()
                .map(|param_assign| match param_assign.kind(db) {
                    ParamAssignKind::NonFormal { value } => {
                        ResolvedParam::new(
                            db,
                            *param_assign,
                            ResolvedParamKind::NonFormal {
                                resolved_param: target
                                    .ty(db)
                                    .ok()
                                    .and_then(|target| target.to_signature(db))
                                    .and_then(|signature| {
                                        // Try to get the param by index
                                        let param = signature.variables.values().nth(formal_index);
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
                            resolved_param: target_ty.and_then(|target| {
                                target.to_signature(db).and_then(|signature| {
                                    signature
                                        .variables
                                        .get(&param.ident)
                                        .or_else(|| {
                                            signature.variables.get(&param.ident).filter(|v| {
                                                v.is_variable_input(db) || v.is_variable_inout(db)
                                            })
                                        })
                                        .map(|p| {
                                            ResolvedVarResult::new(
                                                db,
                                                ResolvedVarOrigin::Formal(param),
                                                ResolvedVarKind::Param(*p),
                                            )
                                        })
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
                            resolved_param: target_ty.and_then(|target| {
                                target.to_signature(db).and_then(|signature| {
                                    signature
                                        .variables
                                        .get(&param.ident)
                                        .filter(|v| v.is_variable_output(db))
                                        .map(|p| {
                                            ResolvedVarResult::new(
                                                db,
                                                ResolvedVarOrigin::Formal(param),
                                                ResolvedVarKind::Param(*p),
                                            )
                                        })
                                })
                            }),
                            variable: resolve_var_access(db, variable),
                        },
                    ),
                })
                .collect(),
        }
    }
}

#[salsa::tracked(debug)]
pub struct ResolvedParam<'db> {
    pub param: ParamAssign<'db>,
    pub kind: ResolvedParamKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedParamKind<'db> {
    NonFormal {
        resolved_param: Option<ResolvedVarResult<'db>>,
        value: ResolvedExpr<'db>,
    },
    FormalInput {
        param: SpanIdent<'db>,
        resolved_param: Option<ResolvedVarResult<'db>>,
        value: ResolvedExpr<'db>,
    },
    FormalOutput {
        not: bool,
        param: SpanIdent<'db>,
        resolved_param: Option<ResolvedVarResult<'db>>,
        variable: ResolvedVarResult<'db>,
    },
}

impl<'db> HirNodeInfo<'db> for ResolvedParam<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.param(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.param(db).scope_id(db)
    }
}
