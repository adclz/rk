use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::variable::VariableDecl,
        scope::FileScopeId,
        visibility::Visibility,
    },
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    #[returns(as_ref)]
    pub extends: Option<SpanNamespaceAccess<'db>>,

    #[returns(ref)]
    pub implements: Vec<SpanNamespaceAccess<'db>>,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub methods: Vec<MethodDecl<'db>>,

    pub modifier: Modifier,

    pub scope_id: FileScopeId<'db>,
}

#[salsa::tracked(debug)]
pub struct MethodDecl<'db> {
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub name: Ident,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    pub modifier: Modifier,

    pub visibility: Visibility,

    pub _override: bool,

    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub stmts: Vec<Stmt<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for MethodDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}
