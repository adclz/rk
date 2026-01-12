use crate::hir_def::{
    expressions::{
        expression::{Expr, InitExpr, PathExpr},
        spec::Spec,
    },
    interned::{identifier::Ident, namespace::SpanNamespaceAccess},
    pous::variable::VariableDecl,
};

#[salsa::tracked(debug)]
pub struct ConfigDecl<'db> {
    name: Ident,

    #[returns(ref)]
    variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    resources: Vec<Resource<'db>>,

    #[returns(ref)]
    access_decls: Vec<AccessDecl<'db>>,

    config_init: ConfigInit<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ConfigInit<'db> {
    config_inst_init: Vec<ConfigInstInit<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ConfigInstInit<'db> {
    path: PathExpr<'db>,

    located_at: Option<VariableDecl<'db>>, // DV

    init: InitExpr<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Resource<'db> {
    Resource(ResourceDecl<'db>),
    Single(SingleResourceDecl<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResourceDecl<'db> {
    name: Ident,

    resource_type_name: Ident,

    variables: Vec<VariableDecl<'db>>,

    resources: Vec<ResourceDecl<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SingleResourceDecl<'db> {
    TaskConfig(TaskConfig<'db>),
    ProgConfig(ProgConfig<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AccessDecl<'db> {
    name: Ident,

    path: AccessPath<'db>,

    access: Spec<'db>,

    direction: AccessDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AccessPath<'db> {
    path: PathExpr<'db>,

    variable: VariableDecl<'db>, // DV
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AccessDirection {
    ReadWrite,
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct TaskConfig<'db> {
    name: Ident,

    init: TaskInit<'db>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct TaskInit<'db> {
    name: Ident,

    single: Option<DataSource<'db>>,
    interval: Option<DataSource<'db>>,

    priority: Expr<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ProgConfig<'db> {
    retain: bool,

    name: Ident,

    task: Option<Ident>,

    access: SpanNamespaceAccess<'db>,

    conf_elements: ProgConfElement<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ProgConfElement<'db> {
    ProgCnxn(ProgCnxn<'db>),
    FbTask(FbTask<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ProgCnxn<'db> {
    ProgDataSource {
        path: PathExpr<'db>,
        source: ProgDataSource<'db>,
    },
    DataSink {
        path: PathExpr<'db>,
        sink: DataSink<'db>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum DataSource<'db> {
    Constant(Expr<'db>),
    PathExpr(PathExpr<'db>),
    Variable(VariableDecl<'db>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ProgDataSource<'db> {
    Constant(Expr<'db>),
    PathExpr(PathExpr<'db>),
    Variable(VariableDecl<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum DataSink<'db> {
    PathExpr(PathExpr<'db>),
    Variable(VariableDecl<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct FbTask<'db> {
    path: PathExpr<'db>,

    task: Ident,
}
