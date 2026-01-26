use std::ops::ControlFlow;

use auto_lsp::{core::span::Span, default::db::file::File};
use db::WorkspaceDataBase;
use hir::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        interned::namespace::SpanNamespaceAccess,
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        scope::ScopeId,
        semantic_index::semantic_index,
        using::Using,
    },
    hir_ty::{signature::inheritance::MethodRef, ty::Type},
};

use crate::to_proto::walk::WalkHir;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpanNamespaceAccessContext<'db> {
    Extends(&'db SpanNamespaceAccess<'db>),
    Implements(&'db SpanNamespaceAccess<'db>),
}

impl<'db> SpanNamespaceAccessContext<'db> {
    pub fn get_access(&self) -> &'db SpanNamespaceAccess<'db> {
        match self {
            SpanNamespaceAccessContext::Extends(access) => access,
            SpanNamespaceAccessContext::Implements(access) => access,
        }
    }
}

impl<'db> HirNodeInfo<'db> for SpanNamespaceAccessContext<'db> {
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.get_access().get_scope_id(db)
    }

    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.get_access().get_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InitExprWithTypeContext<'db> {
    pub init_expr: InitExpr<'db>,
    pub ty: Type<'db>,
}

impl<'db> From<(InitExpr<'db>, Type<'db>)> for InitExprWithTypeContext<'db> {
    fn from((init_expr, ty): (InitExpr<'db>, Type<'db>)) -> Self {
        InitExprWithTypeContext { init_expr, ty }
    }
}

impl<'db> HirNodeInfo<'db> for InitExprWithTypeContext<'db> {
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.init_expr.get_scope_id(db)
    }

    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.init_expr.get_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    SpanNamespaceAccess(SpanNamespaceAccessContext<'db>),
    PouDecl(Pou<'db>),
    VariableDecl(VariableDecl<'db>),
    StructElement(StructElement<'db>),
    Spec(Spec<'db>),
    MethodRef(MethodRef<'db>),
    BeginPathExpr(BeginPathExpr<'db>),
    Expr(Expr<'db>),
    InitExprWithType(InitExprWithTypeContext<'db>),
    PathExpr(PathExpr<'db>),
    VariableAccess(VariableAccess<'db>),
    Using(Using<'db>),
    Param(ParamAssign<'db>),
}

impl<'db> HirNode<'db> {
    pub fn get_span(&'db self, db: &'db dyn WorkspaceDataBase) -> Span {
        match self {
            HirNode::Namespace(n) => n.get_span(db),
            HirNode::SpanNamespaceAccess(s) => s.get_span(db),
            HirNode::PouDecl(p) => p.get_span(db),
            HirNode::VariableDecl(v) => v.get_span(db),
            HirNode::StructElement(s) => s.get_span(db),
            HirNode::Spec(s) => s.get_span(db),
            HirNode::MethodRef(m) => m.get_span(db),
            HirNode::BeginPathExpr(b) => b.get_span(db),
            HirNode::PathExpr(p) => p.get_span(db),
            HirNode::VariableAccess(v) => v.get_span(db),
            HirNode::Expr(e) => e.get_span(db),
            HirNode::InitExprWithType(e) => e.get_span(db),
            HirNode::Using(u) => u.get_span(db),
            HirNode::Param(p) => p.get_span(db),
        }
    }

    pub fn get_scope_id(&'db self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            HirNode::Namespace(n) => n.get_scope_id(db),
            HirNode::SpanNamespaceAccess(s) => s.get_scope_id(db),
            HirNode::PouDecl(p) => p.get_scope_id(db),
            HirNode::VariableDecl(v) => v.get_scope_id(db),
            HirNode::StructElement(s) => s.get_scope_id(db),
            HirNode::Spec(s) => s.get_scope_id(db),
            HirNode::MethodRef(m) => m.get_scope_id(db),
            HirNode::BeginPathExpr(b) => b.get_scope_id(db),
            HirNode::Expr(e) => e.get_scope_id(db),
            HirNode::PathExpr(p) => p.get_scope_id(db),
            HirNode::VariableAccess(v) => v.get_scope_id(db),
            HirNode::InitExprWithType(e) => e.get_scope_id(db),
            HirNode::Using(u) => u.get_scope_id(db),
            HirNode::Param(p) => p.get_scope_id(db),
        }
    }
}

pub fn descendant_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<HirNode<'db>> {
    let mut best_match: Option<HirNode<'db>> = None;

    let _ = semantic_index(db, file).walk_hir(db, &mut |node| {
        let range = node.get_span(db);
        // Only consider nodes that contain the offset
        if range.start_byte <= offset && offset <= range.end_byte {
            // Always update the best match when we find a containing node
            // This ensures we get the deepest (last visited) node in the tree
            best_match = Some(node);
        }
        ControlFlow::Continue(())
    });

    best_match
}
