use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HasPragmas, HirNodeInfo,
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

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> Function<'db> {
    /// The number of formal parameters (VAR_INPUT / VAR_OUTPUT / VAR_IN_OUT),
    /// i.e. the call-site arity. This is the overload discriminant: two
    /// same-named functions are distinct overloads iff their parameter counts
    /// differ, and duplicates iff they match.
    pub fn param_count(&self, db: &'db dyn WorkspaceDataBase) -> usize {
        use crate::hir_def::pous::variable::VariableKind;
        self.variables(db)
            .iter()
            .filter(|v| {
                matches!(
                    v.kind(db),
                    VariableKind::Input | VariableKind::Output | VariableKind::InOut
                )
            })
            .count()
    }
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
