use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::{expression::InitExpr, spec::Spec},
        interned::identifier::Ident,
        scope::FileScopeId,
    },
    {AstId, HirNodeInfo},
};

#[salsa::tracked(debug)]
pub struct VariableDecl<'db> {
    #[returns(ref)]
    pub name: Ident,

    pub kind: VariableKind,

    pub spec: Spec<'db>,

    #[returns(as_ref)]
    pub init: Option<InitExpr<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> VariableDecl<'db> {
    pub fn is_input(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::Input)
    }

    pub fn is_output(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::Output)
    }

    pub fn is_var(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::Var)
    }

    pub fn is_in_out(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::InOut)
    }

    pub fn is_external(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::External)
    }

    pub fn is_global(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::Global)
    }

    pub fn is_access(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::Access)
    }

    pub fn is_temp(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), VariableKind::Temp)
    }

    pub fn is_config(&self, db: &'db dyn BaseDatabase) -> bool {
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

impl<'db> HirNodeInfo<'db> for VariableDecl<'db> {
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
