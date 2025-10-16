use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    check::errors::path_error::PathResolveError, hir_def::{
        expressions::{expression::{FuncCall, ParamAssign}, spec::Spec}, interned::identifier::{Ident, SpanIdent}, pous::variable::VariableDecl, scope::FileScopeId
    }, hir_ty::{
        expr_resolver::ResolvedExpr,
        param_resolver::resolve_parameters,
        signatures::LocalVariables,
        ty::Ty,
        ty_var_access_resolver::{resolve_path_expr, CallSite, ResolvedAccess},
        walk::ResolvedPath,
    }, AstId, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedFuncCall<'db> {
    pub target: ResolvedAccess<'db>,
    pub params: Vec<ResolvedParam<'db>>,
}

impl<'db> ResolvedFuncCall<'db> {
    pub fn to_ty(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Result<Option<Ty<'db>>, PathResolveError<'db>> {
        self.target.resolved(db).and_then(|r| Ok(r.to_ty(db)))
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        self.target.is_callable(db)
    }

    pub fn with_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Spec<'db>> {
        self.target.with_return_type(db)
    }

    pub fn callable(&self, db: &'db dyn BaseDatabase) -> Option<&'db IndexMap<Ident, VariableDecl<'db>>> {
        self.target.callable(db)
    }
}

impl<'db> FuncCall<'db> {
    pub fn resolve_func_call(&self, db: &'db dyn BaseDatabase) -> ResolvedFuncCall<'db> {
        let target = resolve_path_expr(db, self.path);

        ResolvedFuncCall {
            target,
            params: match target.resolved(db) {
                Ok(ResolvedPath::Pou(pou)) => resolve_parameters(db, pou, &self.params),
                Ok(ResolvedPath::Method(method)) => resolve_parameters(db, method, &self.params),
                _ => Default::default(),
            },
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
        resolved_param: Option<ResolvedAccess<'db>>,
        value: ResolvedExpr<'db>,
    },
    FormalInput {
        param: SpanIdent<'db>,
        resolved_param: Option<ResolvedAccess<'db>>,
        value: ResolvedExpr<'db>,
    },
    FormalOutput {
        not: bool,
        param: SpanIdent<'db>,
        resolved_param: Option<ResolvedAccess<'db>>,
        variable: ResolvedAccess<'db>,
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
