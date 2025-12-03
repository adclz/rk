use std::ops::ControlFlow;

use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase, file::File},
};
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        interned::namespace::SpanNamespaceAccessContext,
        namespace::NamespaceDecl,
        pous::{pou::PouDecl, variable::VariableDecl},
        scope::ScopeId,
        semantic_index::{SemanticIndex, semantic_index},
        using::Using,
    },
    hir_ty::inheritance_solver::MethodRef,
};

use crate::to_proto::walk::WalkHir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    SpanNamespaceAccess(SpanNamespaceAccessContext<'db>),
    PouDecl(PouDecl<'db>),
    VariableDecl(VariableDecl<'db>),
    StructElement(StructElement<'db>),
    Spec(Spec<'db>),
    MethodRef(MethodRef<'db>),
    BeginPathExpr(BeginPathExpr<'db>),
    Expr(Expr<'db>),
    InitExpr(InitExpr<'db>),
    PathExpr(PathExpr<'db>),
    VariableAccess(VariableAccess<'db>),
    Using(Using<'db>),
    Param(ParamAssign<'db>),
}

impl<'db> HirNode<'db> {
    pub fn get_span(&'db self, db: &'db dyn BaseDatabase) -> Span {
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
            HirNode::InitExpr(e) => e.get_span(db),
            HirNode::Using(u) => u.get_span(db),
            HirNode::Param(p) => p.get_span(db),
        }
    }

    pub fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
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
            HirNode::InitExpr(e) => e.get_scope_id(db),
            HirNode::Using(u) => u.get_scope_id(db),
            HirNode::Param(p) => p.get_scope_id(db),
        }
    }
}

pub fn descendant_at<'db>(
    db: &'db dyn BaseDatabase,
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
