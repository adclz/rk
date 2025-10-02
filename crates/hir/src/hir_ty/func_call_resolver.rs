use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::expression::{FuncCall, ParamAssign},
        interned::identifier::SpanIdent,
        scope::FileScopeId,
    },
    hir_ty::{
        expr_resolver::ResolvedExpr,
        param_resolver::resolve_parameters,
        ty_path_expr_resolver::resolved_path_expr,
        ty_var_access_resolver::{
            ResolvedVarKind, ResolvedVarOrigin, ResolvedVarResult,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedFuncCall<'db> {
    pub target: ResolvedVarResult<'db>,
    pub params: Vec<ResolvedParam<'db>>,
}

impl<'db> FuncCall<'db> {
    pub fn resolve_func_call(&self, db: &'db dyn BaseDatabase) -> ResolvedFuncCall<'db> {
        let target = ResolvedVarResult::new(
            db,
            ResolvedVarOrigin::PathExpr(self.path),
            ResolvedVarKind::Symbolic(*resolved_path_expr(db, self.path)),
        );

        ResolvedFuncCall {
            target,
            params: target
                .ty(db)
                .ok()
                .map(|ty| resolve_parameters(db, ty, &self.params))
                .unwrap_or_default(),
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
