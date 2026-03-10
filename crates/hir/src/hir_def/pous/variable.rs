use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{InitExpr, Integer},
            spec::Spec,
        },
        interned::identifier::Ident,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct VariableDecl<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    pub kind: VariableKind,

    #[tracked]
    pub variadic: bool,

    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for VariableDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for VariableDecl<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}

impl<'db> VariableDecl<'db> {
    pub fn is_input(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Input)
    }

    pub fn is_output(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Output)
    }

    pub fn is_var(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Var)
    }

    pub fn is_in_out(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::InOut)
    }

    pub fn is_external(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::External)
    }

    pub fn is_global(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Global)
    }

    pub fn is_access(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Access)
    }

    pub fn is_temp(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Temp)
    }

    pub fn is_config(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Config)
    }
}

// VAR Internal to entity (function, function block, etc.)
// VAR_INPUT Externally supplied, not modifiable within entity
// VAR_OUTPUT Supplied by entity to external entities
// VAR_IN_OUT Supplied by external entities, can be modified within entity and supplied to external entity
// VAR_EXTERNAL Supplied by configuration via VAR_GLOBAL
// VAR_GLOBAL Global variable declaration
// VAR_ACCESS Access path declaration
// VAR_TEMP Temporary storage for variables in function blocks, methods and programs
// VAR_CONFIG Instance-specific initialization and location assignment.

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableKind {
    Var,
    Input,
    Output,
    InOut,
    External,
    Global,
    Access,
    Temp,
    Config,
}

#[salsa::tracked(debug)]
pub struct DirectVariable<'db> {
    pub adress: Ident,
    pub partly: bool,
    pub offset: Vec<Integer>,
}

#[salsa::tracked(debug)]
pub struct LocatedVariable<'db> {
    pub name: Option<Ident>,

    pub located_at: DirectVariable<'db>,

    pub spec: Spec<'db>,

    pub init: Option<InitExpr<'db>>,
}
