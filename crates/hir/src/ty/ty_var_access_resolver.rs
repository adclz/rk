
use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        expressions::expression::{VariableAccess, VariableAccessKind},
        scope::FileScopeId,
    },
    ty::{TyResolved, ty::Ty, ty_path_expr_resolver::resolved_path_expr},
};

pub fn resolve_var_access<'db>(
    db: &'db dyn BaseDatabase,
    access: &'db VariableAccess<'db>,
) -> ResolvedVarResult<'db> {
    VarAccessResolverCtx::new(db, access).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedVarResult<'db> {
    #[tracked]
    #[returns(ref)]
    #[no_eq]
    pub origin: VariableAccess<'db>,
    #[tracked]
    #[returns(ref)]
    #[no_eq]
    pub resolved_ty: Option<Ty<'db>>,
}

impl<'db> TyResolved<'db> for ResolvedVarResult<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        *self.resolved_ty(db)
    }
}

pub struct VarAccessResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    access: &'db VariableAccess<'db>,
}

impl<'db> VarAccessResolverCtx<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        access: &'db VariableAccess<'db>,
    ) -> Self {
        Self {
            db,
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
            VariableAccessKind::Symbolic(symbolic) => ResolvedVarResult::new(
                self.db,
                self.access.clone(),
                resolved_path_expr(self.db, symbolic.kind).ty(self.db),
            ),
        }
    }
}
