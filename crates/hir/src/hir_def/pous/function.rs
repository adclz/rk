use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HirNodeInfo,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::identifier::Ident,
        pous::{generics::GenericParam, variable::VariableDecl},
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    pub name: Ident,

    #[tracked]
    pub is_test: bool,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    #[returns(ref)]
    pub generics: Vec<GenericParam<'db>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    #[tracked]
    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Function<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for Function<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}
