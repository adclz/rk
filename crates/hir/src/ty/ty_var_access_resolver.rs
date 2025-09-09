use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::sem_errors::AnalysisError,
    def::{
        expressions::expression::{VariableAccess, VariableAccessKind},
        pous::variable::VariableDecl,
        scope::FileScopeId,
    },
    to_proto::{AstId, ToProto},
    ty::{
        TyInfo,
        ty::{Ty, TyDecl, TyKind},
        ty_path_expr_resolver::{ResolvedPathResult, resolved_path_expr},
    },
};

pub fn resolve_var_access<'db>(
    db: &'db dyn BaseDatabase,
    access: &'db VariableAccess<'db>,
) -> ResolvedVarResult<'db> {
    VarAccessResolverCtx::new(db, access).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedVarResult<'db> {
    pub origin: VariableAccess<'db>,

    pub kind: ResolvedVarKind<'db>,
}

impl<'db> ResolvedVarResult<'db> {
    fn get_var(&self, db: &'db dyn BaseDatabase) -> Option<VariableDecl<'db>> {
        self.ty(db).ok().and_then(|ty| {
            if let TyDecl::Variable(var) = ty.decl(db) {
                Some(var)
            } else {
                None
            }
        })
    }

    pub fn is_input(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_input(db))
    }

    pub fn is_output(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_output(db))
    }

    pub fn is_var(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_var(db))
    }

    pub fn is_in_out(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_in_out(db))
    }

    pub fn is_external(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_external(db))
    }

    pub fn is_global(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_global(db))
    }

    pub fn is_access(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_access(db))
    }

    pub fn is_temp(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_temp(db))
    }

    pub fn is_config(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).map_or(false, |var| var.is_config(db))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedVarKind<'db> {
    Direct,
    Symbolic(ResolvedPathResult<'db>),
}

impl<'db> TyInfo<'db> for ResolvedVarResult<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AnalysisError<'db>> {
        match self.kind(db) {
            ResolvedVarKind::Direct => todo!(), // todo: direct var type
            ResolvedVarKind::Symbolic(ref path) => path.ty(db),
        }
    }
    fn place(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.origin(db).id(db)
    }
}

pub struct VarAccessResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    access: &'db VariableAccess<'db>,
}

impl<'db> VarAccessResolverCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, access: &'db VariableAccess<'db>) -> Self {
        Self { db, access }
    }

    pub fn resolve(&self) -> ResolvedVarResult<'db> {
        match &self.access.kind(self.db) {
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
                *self.access,
                ResolvedVarKind::Symbolic(*resolved_path_expr(self.db, symbolic.kind)),
            ),
        }
    }
}

impl<'db> ToProto<'db> for ResolvedVarResult<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.origin(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.origin(db).scope_id(db)
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        if let Ok(ty) = self.ty(db) {
            ty.declaration(db)
        } else {
            None
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        if let Ok(ty) = self.ty(db) {
            ty.definition(db)
        } else {
            None
        }
    }
}
