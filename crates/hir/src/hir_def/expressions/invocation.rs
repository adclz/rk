use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::expression::{ParamAssign, PathExpr},
        scope::FileScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Invocation<'db> {
    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub keyword_id: AstId,

    pub scope_id: FileScopeId<'db>,

    pub kind: InvocationKind<'db>,

    pub params: Vec<ParamAssign<'db>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InvocationKind<'db> {
    This { path: PathExpr<'db> },
    Super { path: PathExpr<'db> },
    SuperBody,
}

impl<'db> HirNodeInfo<'db> for Invocation<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}
