use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HasPragmas, HirNodeInfo, Visibility,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::identifier::Ident,
        pous::{pragma::Pragma, variable::VariableDecl},
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    pub name: Ident,

    #[tracked]
    #[returns(ref)]
    pub pragmas: Vec<Pragma<'db>>,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

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

    /// `FUNCTION PRIVATE f`: reachable only from its own namespace (nested
    /// ones included) on its own side of the library line. Empty when the
    /// header writes no specifier.
    #[tracked]
    pub visibility: Visibility,

    /// The specifier's node, so a misplaced one is reported at the keyword.
    #[tracked]
    #[no_eq]
    pub spec_id: Option<AstId>,

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

impl<'db> HasPragmas<'db> for Function<'db> {
    fn get_pragmas(&self, db: &'db dyn WorkspaceDataBase) -> &'db [Pragma<'db>] {
        self.pragmas(db)
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
