use db::WorkspaceDataBase;

use crate::hir_def::{
    expressions::{
        expression::{Expr, InitExpr, PathExpr},
        spec::Spec,
    },
    interned::identifier::{Ident, SpanIdent},
    pous::variable::{DirectVariable, VariableDecl},
    scope::ScopeId,
};
use crate::{AstId, HasName, HirNodeInfo};

#[salsa::tracked(debug)]
pub struct ConfigDecl<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_span: AstId,

    #[tracked]
    #[no_eq]
    pub span: AstId,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub resources: Vec<ResourceDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub access_decls: Vec<AccessDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub config_init: Vec<ConfigInstInit<'db>>,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

/// A `RESOURCE ... END_RESOURCE` block: a named group of tasks and the
/// programs bound to them.
///
/// It holds no variables — `VAR_GLOBAL` is application-scoped, declared on the
/// CONFIGURATION — and no scope of its own. Its name is what a deployment
/// binds to an execution unit.
#[salsa::tracked(debug)]
pub struct ResourceDecl<'db> {
    pub name: SpanIdent<'db>,

    #[tracked]
    pub resource_type_name: Ident,

    #[tracked]
    #[returns(ref)]
    pub tasks: Vec<TaskConfig<'db>>,

    #[tracked]
    #[returns(ref)]
    pub programs: Vec<ProgConfig<'db>>,

    #[tracked]
    #[no_eq]
    pub span: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for ResourceDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.span(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

/// Merged from the old `TaskConfig` + `TaskInit` pair.
#[salsa::tracked(debug)]
pub struct TaskConfig<'db> {
    pub name: SpanIdent<'db>,

    #[tracked]
    pub single: Option<DataSource<'db>>,

    #[tracked]
    pub interval: Option<DataSource<'db>>,

    /// Priority value — stored as an `Ident` holding the integer text (e.g. `"5"`).
    /// `None` when PRIORITY is missing from the TASK init (E1405 is emitted).
    #[tracked]
    pub priority: Option<Ident>,

    #[tracked]
    #[no_eq]
    pub span: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for TaskConfig<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.span(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[salsa::tracked(debug)]
pub struct ProgConfig<'db> {
    pub name: SpanIdent<'db>,

    /// Config-level retain qualifier: `Some(true)` for `PROGRAM RETAIN ...`,
    /// `Some(false)` for `PROGRAM NON_RETAIN ...`, `None` when absent (the
    /// program declaration's own qualifiers decide).
    #[tracked]
    pub retain: Option<bool>,

    /// Optional task name from `WITH <task>`, with span for diagnostics.
    #[tracked]
    pub task: Option<SpanIdent<'db>>,

    /// Reference to the program type (e.g. `MyProgram` or `NS::MyProgram`).
    #[tracked]
    pub prog_type: Spec<'db>,

    #[tracked]
    #[returns(ref)]
    pub conf_elements: Vec<ProgConfElement<'db>>,

    #[tracked]
    #[no_eq]
    pub span: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for ProgConfig<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.span(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ProgConfElement<'db> {
    Connection(ProgCnxn<'db>),
    FbTask(FbTask<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ProgCnxn<'db> {
    /// `path := source` — assigns a data source to a variable path.
    Source {
        path: PathExpr<'db>,
        source: DataSource<'db>,
    },
    /// `path => sink` — connects a variable path to a data sink.
    Sink {
        path: PathExpr<'db>,
        sink: DataSink<'db>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum DataSource<'db> {
    Constant(Expr<'db>),
    Path(PathExpr<'db>),
    Direct(DirectVariable<'db>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum DataSink<'db> {
    Path(PathExpr<'db>),
    Direct(DirectVariable<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct FbTask<'db> {
    pub path: PathExpr<'db>,

    /// The task named, with its span for the IDE.
    pub task: SpanIdent<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AccessDecl<'db> {
    pub name: Ident,

    pub path: AccessPath<'db>,

    pub access: Spec<'db>,

    pub direction: AccessDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AccessPath<'db> {
    pub path: PathExpr<'db>,

    /// Optional `direct_variable` (AT address) suffix on the access path.
    pub direct: Option<DirectVariable<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AccessDirection {
    ReadWrite,
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ConfigInstInit<'db> {
    pub path: PathExpr<'db>,

    /// Optional AT address (located variable).
    pub located_at: Option<DirectVariable<'db>>,

    /// The type the entry writes, which repeats the variable's own.
    pub spec: Option<crate::hir_def::expressions::spec::Spec<'db>>,

    /// Absent for a location-only entry (`AT %QB25 : BYTE;`), the standard's
    /// own form; an entry has a location, a value, or both.
    pub init: Option<InitExpr<'db>>,
}

impl<'db> HirNodeInfo<'db> for ConfigDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.span(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> crate::hir_def::scope::ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for ConfigDecl<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_span(db)
    }
}
