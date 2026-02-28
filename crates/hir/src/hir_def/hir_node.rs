use db::WorkspaceDataBase;
use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            invocation::Invocation,
            spec::{Spec, StructElement},
        },
        interned::namespace::SpanNamespaceAccess,
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
        scope::ScopeId,
        using::Using,
    },
    hir_ty::head::inheritance::MethodRef,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    Using(Using<'db>),
    PouDecl(Pou<'db>),
    Program(ProgramDecl<'db>),
    NamespaceAccess(SpanNamespaceAccess<'db>),
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
}

impl<'db> HirNodeInfo<'db> for HirNode<'db> {
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            HirNode::Namespace(n) => n.get_scope_id(db),
            HirNode::NamespaceAccess(n) => n.get_scope_id(db),
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
        }
    }

    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            HirNode::Namespace(n) => n.get_id(db),
            HirNode::NamespaceAccess(n) => n.get_id(db),
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
        }
    }
}
