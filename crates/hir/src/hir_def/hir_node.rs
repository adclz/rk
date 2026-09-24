use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        config::{ConfigDecl, ProgConfig, ResourceDecl, TaskConfig},
        expressions::{
            expression::{Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            invocation::Invocation,
            spec::{Spec, StructElement},
        },
        namespace::NamespaceDecl,
        pous::{
            pou::Pou,
            variable::{DirectVariable, VariableDecl},
        },
        program::ProgramDecl,
        scope::ScopeId,
        using::Using,
    },
    hir_ty::head::inheritance::MethodRef,
};
use db::WorkspaceDataBase;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    Using(Using<'db>),
    Config(ConfigDecl<'db>),
    Resource(ResourceDecl<'db>),
    Task(TaskConfig<'db>),
    ProgConfig(ProgConfig<'db>),
    PouDecl(Pou<'db>),
    Program(ProgramDecl<'db>),
    MethodRef(MethodRef<'db>),
    VariableDecl(VariableDecl<'db>),
    Spec(Spec<'db>),
    StructElement(StructElement<'db>),
    VariableAccess(VariableAccess<'db>),
    Invocation(Invocation<'db>),
    Expr(Expr<'db>),
    Param(ParamAssign<'db>),
    InitExpr(InitExpr<'db>),
    PathExpr(PathExpr<'db>),
    /// An address written where no access wraps it: an `AT` clause, a
    /// VAR_CONFIG entry, a connection.
    DirectVariable(DirectVariable<'db>),
}

impl<'db> HirNodeInfo<'db> for HirNode<'db> {
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            HirNode::Namespace(n) => n.get_scope_id(db),
            HirNode::Config(c) => c.get_scope_id(db),
            HirNode::Resource(r) => r.get_scope_id(db),
            HirNode::Task(t) => t.get_scope_id(db),
            HirNode::ProgConfig(p) => p.get_scope_id(db),
            HirNode::PouDecl(p) => p.get_scope_id(db),
            HirNode::Program(p) => p.get_scope_id(db),
            HirNode::VariableDecl(v) => v.get_scope_id(db),
            HirNode::StructElement(s) => s.get_scope_id(db),
            HirNode::Spec(s) => s.get_scope_id(db),
            HirNode::MethodRef(m) => m.get_scope_id(db),
            HirNode::Expr(e) => e.get_scope_id(db),
            HirNode::PathExpr(p) => p.get_scope_id(db),
            HirNode::VariableAccess(v) => v.get_scope_id(db),
            HirNode::Using(u) => u.get_scope_id(db),
            HirNode::Param(p) => p.get_scope_id(db),
            HirNode::InitExpr(i) => i.get_scope_id(db),
            HirNode::Invocation(i) => i.get_scope_id(db),
            HirNode::DirectVariable(d) => d.get_scope_id(db),
        }
    }

    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            HirNode::Namespace(n) => n.get_id(db),
            HirNode::Config(c) => c.get_id(db),
            HirNode::Resource(r) => r.get_id(db),
            HirNode::Task(t) => t.get_id(db),
            HirNode::ProgConfig(p) => p.get_id(db),
            HirNode::PouDecl(p) => p.get_id(db),
            HirNode::Program(p) => p.get_id(db),
            HirNode::VariableDecl(v) => v.get_id(db),
            HirNode::StructElement(s) => s.get_id(db),
            HirNode::Spec(s) => s.get_id(db),
            HirNode::MethodRef(m) => m.get_id(db),
            HirNode::Expr(e) => e.get_id(db),
            HirNode::PathExpr(p) => p.get_id(db),
            HirNode::VariableAccess(v) => v.get_id(db),
            HirNode::Using(u) => u.get_id(db),
            HirNode::Param(p) => p.get_id(db),
            HirNode::InitExpr(i) => i.get_id(db),
            HirNode::Invocation(i) => i.get_id(db),
            HirNode::DirectVariable(d) => d.get_id(db),
        }
    }
}
