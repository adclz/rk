use ast::generated::DByteStrSpec_DChar_SByteStrSpec_SChar;
use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::sem_errors::AnalysisError,
    def::{
        expressions::expression::{PathExpr, VariableAccess, VariableAccessKind},
        scope::FileScopeId,
    },
    to_proto::AstId,
    ty::{
        TyInfo,
        ty::{Ty, TyOrigin},
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedVarKind<'db> {
    Direct,
    Symbolic(ResolvedPathResult<'db>),
}

impl<'db> TyInfo<'db> for ResolvedVarResult<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self.kind(db) {
            ResolvedVarKind::Direct => None, // todo: direct var type
            ResolvedVarKind::Symbolic(path) => path.ty(db),
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
