use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    check::errors::var_error::VarResolveError,
    hir_def::{
        expressions::{
            expression::{Expr, PathExpr, VariableAccess, VariableAccessKind},
            invocation::{Invocation, InvocationKind},
        },
        interned::identifier::SpanIdent,
        pous::variable::VariableDecl,
        scope::FileScopeId,
    },
    hir_ty::{
        ty::{Ty, TyDecl},
        ty_path_expr_resolver::{ResolvedPathResult, resolved_path_expr},
    },
};

pub fn resolve_var_access<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
) -> ResolvedVarResult<'db> {
    VarAccessResolverCtx::new(db, access).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedVarResult<'db> {
    pub origin: ResolvedVarOrigin<'db>,

    pub kind: ResolvedVarKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedVarOrigin<'db> {
    // todo: add keyword origin
    InvocationKeyword(AstId, Invocation<'db>),
    Invocation(Invocation<'db>),
    Access(VariableAccess<'db>),
    NonFormal(Expr<'db>),
    Formal(SpanIdent<'db>),
    PathExpr(PathExpr<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedVarKind<'db> {
    Direct,
    Pou(Ty<'db>),
    Method(Ty<'db>),
    Symbolic(ResolvedPathResult<'db>),
    Param(Ty<'db>),
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
        self.get_var(db).is_some_and(|var| var.is_input(db))
    }

    pub fn is_output(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_output(db))
    }

    pub fn is_var(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_var(db))
    }

    pub fn is_in_out(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_in_out(db))
    }

    pub fn is_external(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_external(db))
    }

    pub fn is_global(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_global(db))
    }

    pub fn is_access(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_access(db))
    }

    pub fn is_temp(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_temp(db))
    }

    pub fn is_config(&self, db: &'db dyn BaseDatabase) -> bool {
        self.get_var(db).is_some_and(|var| var.is_config(db))
    }
}

impl<'db> ResolvedVarResult<'db> {
    pub fn ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, VarResolveError<'db>> {
        match self.kind(db) {
            ResolvedVarKind::Direct => todo!(), // todo: direct var type
            ResolvedVarKind::Pou(ty) => Ok(ty),
            ResolvedVarKind::Method(ty) => Ok(ty),
            ResolvedVarKind::Symbolic(path) => path
                .ty(db)
                .map_err(|err| VarResolveError::PathResolveError { err }),
            ResolvedVarKind::Param(ty) => Ok(ty),
        }
    }
}

pub struct VarAccessResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
}

impl<'db> VarAccessResolverCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, access: VariableAccess<'db>) -> Self {
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
                ResolvedVarOrigin::Access(self.access),
                ResolvedVarKind::Symbolic(*resolved_path_expr(self.db, symbolic.kind)),
            ),
        }
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedVarResult<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self.origin(db) {
            ResolvedVarOrigin::Access(access) => access.get_id(db),
            ResolvedVarOrigin::NonFormal(expr) => expr.get_id(db),
            ResolvedVarOrigin::Formal(param) => param.get_id(db),
            ResolvedVarOrigin::Invocation(ty) => match ty.kind(db) {
                InvocationKind::This { path } => path.get_id(db),
                InvocationKind::Super { path } => path.get_id(db),
                InvocationKind::SuperBody => ty.get_id(db),
            },
            ResolvedVarOrigin::InvocationKeyword(id, _) => id,
            ResolvedVarOrigin::PathExpr(path) => path.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self.origin(db) {
            ResolvedVarOrigin::Access(access) => access.get_scope_id(db),
            ResolvedVarOrigin::NonFormal(expr) => expr.get_scope_id(db),
            ResolvedVarOrigin::Formal(param) => param.get_scope_id(db),
            ResolvedVarOrigin::Invocation(ty) => ty.get_scope_id(db),
            ResolvedVarOrigin::InvocationKeyword(id, scope) => scope.get_scope_id(db),
            ResolvedVarOrigin::PathExpr(path) => path.get_scope_id(db),
        }
    }
}
