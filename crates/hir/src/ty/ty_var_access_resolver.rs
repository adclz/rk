use ast::generated::DByteStrSpec_DChar_SByteStrSpec_SChar;
use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::sem_errors::AnalysisError,
    def::{
        expressions::expression::{PathExpr, VariableAccess, VariableAccessKind},
        scope::FileScopeId,
    },
    to_proto::{AstId, ToProto},
    ty::{
        ty::{Ty, TyDecl}, ty_path_expr_resolver::{resolved_path_expr, ResolvedPathResult}, TyInfo
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedVarKind<'db> {
    Direct,
    Symbolic(ResolvedPathResult<'db>),
}

impl<'db> TyInfo<'db> for ResolvedVarResult<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self.kind(db) {
            ResolvedVarKind::Direct => None, // todo: direct var type
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
        if let Some(ty) = self.ty(db) {
            ty.declaration(db)
        } else {
            None
        }
    }

    fn definition(
            &'db self,
            db: &'db dyn BaseDatabase,
        ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        if let Some(ty) = self.ty(db) {
            ty.definition(db)
        } else {
            None
        }
    }
}