use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HirNodeInfo, Modifier,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::identifier::Ident,
        pous::{class::MethodDecl, variable::VariableDecl},
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<Spec<'db>>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<Spec<'db>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub methods: Vec<MethodDecl<'db>>,

    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    #[tracked]
    pub modifier: Modifier,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for FunctionBlock<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for FunctionBlock<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}
