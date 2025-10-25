use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    check::errors::path_error::AccessError, hir_def::{
        expressions::{
            expression::{Expr, FuncCall, ParamAssign, VariableAccess},
            spec::Spec,
        },
        interned::identifier::{Ident, SpanIdent},
        pous::variable::VariableDecl,
        scope::ScopeId,
    }, hir_ty::{
        expr_resolver::ResolvedExpr,
        ty::Ty,
        ty_var_access_resolver::{resolve_global_path_expr, ResolvedAccess},
        walk::{ResolvedPath, ResolvedPathKind},
    }, AstId, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedFuncCall<'db> {
    pub target: ResolvedAccess<'db>,
    pub params: Vec<ParamAssign<'db>>,
}

impl<'db> ResolvedFuncCall<'db> {
    pub fn to_ty(&'db self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AccessError<'db>> {
        self.target.fully_resolved(db).and_then(|r| r.try_to_ty(db))
    }

    pub fn with_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Spec<'db>> {
        self.target.with_return_type(db)
    }

    pub fn callable(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> Option<&'db IndexMap<Ident, VariableDecl<'db>>> {
        self.target.callable(db)
    }
}

impl<'db> FuncCall<'db> {
    pub fn resolve_func_call(&self, db: &'db dyn BaseDatabase) -> ResolvedFuncCall<'db> {
        let target = resolve_global_path_expr(db, self.path(db));

        ResolvedFuncCall {
            target: target.clone(),
            params: match target.fully_resolved(db) {
                Ok(ResolvedPath {
                    kind: ResolvedPathKind::Pou(pou),
                    ..
                }) => self.params(db).clone(),
                Ok(ResolvedPath {
                    kind: ResolvedPathKind::Method(method),
                    ..
                }) => self.params(db).clone(),
                _ => Default::default(),
            },
        }
    }
}
