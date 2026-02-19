use crate::AstId;
use crate::hir_def::{
    expressions::{
        expression::{Expr, InitExpr, PathExpr},
        spec::Spec,
    },
    interned::{identifier::Ident, namespace::SpanNamespaceAccess},
    pous::variable::{DirectVariable, VariableDecl},
    scope::ScopeId,
};

#[salsa::tracked(debug)]
pub struct ConfigDecl<'db> {
    pub name: Ident,

    pub span: AstId,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub resources: Vec<ConfigResource<'db>>,

    #[returns(ref)]
    pub access_decls: Vec<AccessDecl<'db>>,

    #[returns(ref)]
    pub config_init: Vec<ConfigInstInit<'db>>,

    pub scope_id: ScopeId<'db>,
}

/// A resource entry at the CONFIGURATION level.
///
/// A configuration can contain either a full `RESOURCE...END_RESOURCE` block or bare
/// `TASK`/`PROGRAM` declarations at the top level.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConfigResource<'db> {
    /// A named `RESOURCE identifier ON type ... END_RESOURCE` block.
    Resource(ResourceDecl<'db>),
    /// A bare `TASK` declaration at the configuration level.
    Task(TaskConfig<'db>),
    /// A bare `PROGRAM` declaration at the configuration level.
    Program(ProgConfig<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResourceDecl<'db> {
    pub name: Ident,

    pub resource_type_name: Ident,

    /// VAR_GLOBAL variables declared inside this resource block.
    pub variables: Vec<VariableDecl<'db>>,

    pub tasks: Vec<TaskConfig<'db>>,

    pub programs: Vec<ProgConfig<'db>>,
}

/// Merged from the old `TaskConfig` + `TaskInit` pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct TaskConfig<'db> {
    pub name: Ident,

    pub single: Option<DataSource<'db>>,

    pub interval: Option<DataSource<'db>>,

    /// Priority value — stored as an `Ident` holding the integer text (e.g. `"5"`).
    pub priority: Ident,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ProgConfig<'db> {
    pub retain: bool,

    pub name: Ident,

    /// Optional task name from `WITH <task>`.
    pub task: Option<Ident>,

    /// Reference to the program type (e.g. `MyProgram` or `NS::MyProgram`).
    pub prog_type: SpanNamespaceAccess<'db>,

    pub conf_elements: Vec<ProgConfElement<'db>>,
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

    pub task: Ident,
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

    pub init: InitExpr<'db>,
}
