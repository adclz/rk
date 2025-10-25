use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{expressions::{expression::{Expr, FuncCall, ParamAssign, ParamAssignKind, VariableAccess}, invocation::Invocation}, interned::identifier::SpanIdent, pous::{pou::PouDecl, variable::VariableDecl}, scope::ScopeId}, hir_ty::{
        expr_resolver::resolve_expr, func_call_resolver::{}, inheritance_solver::MethodRef, signatures::LocalVariables, ty_var_access_resolver::{resolve_var_access, CallSite, ResolvedAccess}, walk::{Adjustement, ResolvedPath, ResolvedPathKind, ResolvedPathResult}
    }, AstId, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedParam<'db> {
    pub param_assign: ParamAssign<'db>,
    pub kind: ResolvedParamKind<'db>,
}

impl<'db> ResolvedParam<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        param_assign: ParamAssign<'db>,
        kind: ResolvedParamKind<'db>,
    ) -> Self {
        ResolvedParam { param_assign, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedParamKind<'db> {
    NonFormal {
        resolved_param: Option<VariableDecl<'db>>,
        value: Expr<'db>,
    },
    FormalInput {
        param: SpanIdent<'db>,
        resolved_param: Option<VariableDecl<'db>>,
        value: Expr<'db>,
    },
    FormalOutput {
        not: bool,
        param: SpanIdent<'db>,
        resolved_param: Option<VariableDecl<'db>>,
        variable: VariableAccess<'db>,
    },
}

impl<'db> HirNodeInfo<'db> for ResolvedParam<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.param_assign.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.param_assign.scope_id(db)
    }
}

pub fn resolve_parameters<'db>(
    db: &'db dyn BaseDatabase,
    callee: &impl LocalVariables<'db>,
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
                                *p
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
                                *p
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
                                *p
                            })
                    },
                    variable,
                },
            ),
        })
        .collect()
}

