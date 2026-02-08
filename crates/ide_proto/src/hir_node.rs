use db::WorkspaceDataBase;
use hir::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, ParamAssign, ParamAssignKind, PathExpr, VariableAccess},
            invocation::Invocation,
            spec::{Spec, StructElement},
        },
        interned::namespace::SpanNamespaceAccess,
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        scope::ScopeId,
        using::Using,
    },
    hir_ty::{head::inheritance::MethodRef, ty::Type},
};

use crate::comment_index::comment_index;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathExprRoot<'db> {
    VariableAccess(VariableAccess<'db>),
    Invocation(Invocation<'db>),
    PathExpr(PathExpr<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    Using(Using<'db>),
    PouDecl(Pou<'db>),
    NamespaceAccess(SpanNamespaceAccess<'db>),
    MethodRef(MethodRef<'db>),
    VariableDecl(VariableDecl<'db>),
    Spec(Spec<'db>),
    StructElement(StructElement<'db>),
    VariableAccess(VariableAccess<'db>),
    Invocation(Invocation<'db>),
    Expr(Expr<'db>),
    Param(ParamAssign<'db>),
    InitExpr {
        prev: InitExpr<'db>,
        curr: InitExpr<'db>,
    },
    PathExpr {
        prev: PathExprRoot<'db>,
        curr: PathExpr<'db>,
    },
}

impl<'db> HirNodeInfo<'db> for HirNode<'db> {
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            HirNode::Namespace(n) => n.get_scope_id(db),
            HirNode::NamespaceAccess(n) => n.get_scope_id(db),
            HirNode::PouDecl(p) => p.get_scope_id(db),
            HirNode::VariableDecl(v) => v.get_scope_id(db),
            HirNode::StructElement(s) => s.get_scope_id(db),
            HirNode::Spec(s) => s.get_scope_id(db),
            HirNode::MethodRef(m) => m.get_scope_id(db),
            HirNode::Expr(e) => e.get_scope_id(db),
            HirNode::PathExpr { curr, .. } => curr.get_scope_id(db),
            HirNode::VariableAccess(v) => v.get_scope_id(db),
            HirNode::Using(u) => u.get_scope_id(db),
            HirNode::Param(p) => p.get_scope_id(db),
            HirNode::InitExpr { curr, .. } => curr.get_scope_id(db),
            HirNode::Invocation(i) => i.get_scope_id(db),
        }
    }

    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            HirNode::Namespace(n) => n.get_id(db),
            HirNode::NamespaceAccess(n) => n.get_id(db),
            HirNode::PouDecl(p) => p.get_id(db),
            HirNode::VariableDecl(v) => v.get_id(db),
            HirNode::StructElement(s) => s.get_id(db),
            HirNode::Spec(s) => s.get_id(db),
            HirNode::MethodRef(m) => m.get_id(db),
            HirNode::Expr(e) => e.get_id(db),
            HirNode::PathExpr { curr, .. } => curr.get_id(db),
            HirNode::VariableAccess(v) => v.get_id(db),
            HirNode::Using(u) => u.get_id(db),
            HirNode::Param(p) => p.get_id(db),
            HirNode::InitExpr { curr, .. } => curr.get_id(db),
            HirNode::Invocation(i) => i.get_id(db),
        }
    }
}

pub trait MaybeHirNode<'db> {
    fn as_hir_node(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<&'db dyn HirNodeInfo<'db>>;
}

impl<'db> MaybeHirNode<'db> for Type<'db> {
    fn as_hir_node(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
    ) -> Option<&'db dyn HirNodeInfo<'db>> {
        match self {
            Type::Function(f) => Some(f),
            Type::FunctionBlock(fb) => Some(fb),
            Type::Class(c) => Some(c),
            Type::Interface(i) => Some(i),
            Type::DataType(dt) => Some(dt),
            Type::Variable((v, _)) => Some(v),
            Type::StructElement(st) => Some(st),
            _ => None,
        }
    }
}

pub trait HasComment<'db>: HirNodeInfo<'db> {
    fn get_comment(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<String> {
        let comment = match comment_index(db, self.get_scope_id(db).file(db)).find_nearby_comment(
            self.get_scope_id(db).file(db).document(db),
            &self.get_span(db),
        ) {
            Some(c) => c.to_string(self.get_scope_id(db).file(db).document(db)),
            None => "".to_string(),
        };
        Some(comment)
    }
}

impl<'db, T> HasComment<'db> for T where T: HirNodeInfo<'db> + ?Sized {}

pub fn get_param_start_pos<'db>(
    db: &'db dyn WorkspaceDataBase,
    param: &'db ParamAssign<'db>,
) -> Box<dyn HirNodeInfo<'db> + 'db> {
    match param.kind(db) {
        ParamAssignKind::FormalInput { param, .. } => Box::new(param) as _,
        ParamAssignKind::FormalOutput { param, .. } => Box::new(param) as _,
        ParamAssignKind::NonFormal { value } => Box::new(value) as _,
    }
}
