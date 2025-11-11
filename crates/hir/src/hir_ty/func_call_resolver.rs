use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::path_error::AccessError, hir_def::{
        expressions::{
            expression::{FuncCall, ParamAssign},
            spec::Spec,
        },
        interned::identifier::Ident,
        pous::variable::VariableDecl,
    }, hir_ty::{
        def_map::FxIndexMap, ty::Ty, ty_var_access_resolver::{LookUp, ResolvedAccess}, walk::{ResolvedPath, ResolvedPathKind}
    }
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

    pub fn callable(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> Option<&'db FxIndexMap<Ident, VariableDecl<'db>>> {
        self.target.callable(db)
    }
}

impl<'db> FuncCall<'db> {
    pub fn resolve_func_call(&self, db: &'db dyn BaseDatabase) -> ResolvedFuncCall<'db> {
        let target = self.path(db).lookup(db);

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
