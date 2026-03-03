use db::WorkspaceDataBase;

use crate::{
    AstId, HasModifiers, HasName, HirNodeInfo, Modifier, Visibility,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::identifier::Ident,
        pous::variable::VariableDecl,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[returns(as_ref)]
    pub extends: Option<Spec<'db>>,

    #[returns(ref)]
    pub implements: Vec<Spec<'db>>,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub methods: Vec<MethodDecl<'db>>,

    pub modifier: Modifier,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Class<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for Class<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}

impl<'db> HasModifiers<'db> for Class<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        self.modifier(db)
    }
}

#[salsa::tracked(debug)]
pub struct MethodDecl<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

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

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for MethodDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for MethodDecl<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}
