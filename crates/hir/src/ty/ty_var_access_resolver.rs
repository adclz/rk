use std::sync::Arc;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        expressions::expression::{VariableAccess, VariableAccessKind},
        scope::FileScopeId,
    },
    ty::{TyResolved, ty::Ty, ty_path_expr_resolver::resolved_path_expr},
};

#[salsa::tracked(no_eq)]
pub fn resolve_var_access<'db>(
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId,
    access: &'db VariableAccess<'db>,
) -> Arc<ResolvedVarResult<'db>> {
    Arc::new(VarAccessResolverCtx::new(db, scope_id, access).resolve())
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedVarResult<'db> {
    pub origin: VariableAccess<'db>,
    pub ty: Option<Ty<'db>>,
}

impl<'db> TyResolved<'db> for ResolvedVarResult<'db> {
    fn ty(&self) -> Option<Ty<'db>> {
        self.ty
    }
}

pub struct VarAccessResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId,
    access: &'db VariableAccess<'db>,
}

impl<'db> VarAccessResolverCtx<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        scope_id: FileScopeId,
        access: &'db VariableAccess<'db>,
    ) -> Self {
        Self {
            db,
            scope_id,
            access,
        }
    }

    pub fn resolve(&self) -> ResolvedVarResult<'db> {
        match &self.access.kind {
            VariableAccessKind::Direct {
                adress,
                partly,
                offset,
            } => {
                // tododododo asap
                todo!()
            }
            VariableAccessKind::Symbolic(symbolic) => ResolvedVarResult {
                origin: self.access.clone(),
                ty: resolved_path_expr(self.db, self.scope_id.file(), symbolic.kind).ty(),
            },
        }
    }
}
