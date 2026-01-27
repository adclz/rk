use std::ops::ControlFlow;

use auto_lsp::{core::{document_symbols_builder::DocumentSymbolsBuilder, semantic_tokens_builder::SemanticTokensBuilder, span::Span}, default::db::file::File, lsp_types::{CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint, request::{GotoDeclarationResponse, GotoImplementationResponse}}};
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

use crate::{comment_index::comment_index};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    PouDecl(Pou<'db>),
    VariableDecl(VariableDecl<'db>),
    StructElement(StructElement<'db>),
    Spec(Spec<'db>),
    MethodRef(MethodRef<'db>),
    BeginPathExpr(BeginPathExpr<'db>),
    Expr(Expr<'db>),
    PathExpr(PathExpr<'db>),
    VariableAccess(VariableAccess<'db>),
    Using(Using<'db>),
    Param(ParamAssign<'db>),
    InitExpr(InitExpr<'db>),
}

impl<'db> HirNode<'db> {
    pub fn document_symbols(
        &self,
        _db: &'db dyn WorkspaceDataBase,
        _builder: &mut DocumentSymbolsBuilder,
    ) {
    }

    pub fn completion(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    pub fn code_lens(&self, _db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        None
    }

    pub fn implementation(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        None
    }

    pub fn inlay_hint(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        None
    }

    pub fn hover(&'db self, _db: &'db dyn WorkspaceDataBase, _offset: usize) -> Option<Hover> {
        None
    }

    pub fn declaration(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        None
    }

    pub fn definition(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        None
    }

    pub fn semantic_tokens(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _builder: &mut SemanticTokensBuilder,
    ) {
    }
}

impl<'db> HirNodeInfo<'db> for HirNode<'db> {
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            HirNode::Namespace(n) => n.get_scope_id(db),
            HirNode::PouDecl(p) => p.get_scope_id(db),
            HirNode::VariableDecl(v) => v.get_scope_id(db),
            HirNode::StructElement(s) => s.get_scope_id(db),
            HirNode::Spec(s) => s.get_scope_id(db),
            HirNode::MethodRef(m) => m.get_scope_id(db),
            HirNode::BeginPathExpr(b) => b.get_scope_id(db),
            HirNode::Expr(e) => e.get_scope_id(db),
            HirNode::PathExpr(p) => p.get_scope_id(db),
            HirNode::VariableAccess(v) => v.get_scope_id(db),
            HirNode::Using(u) => u.get_scope_id(db),
            HirNode::Param(p) => p.get_scope_id(db),
            HirNode::InitExpr(i) => i.get_scope_id(db),
        }
    }

    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            HirNode::Namespace(n) => n.get_id(db),
            HirNode::PouDecl(p) => p.get_id(db),
            HirNode::VariableDecl(v) => v.get_id(db),
            HirNode::StructElement(s) => s.get_id(db),
            HirNode::Spec(s) => s.get_id(db),
            HirNode::MethodRef(m) => m.get_id(db),
            HirNode::BeginPathExpr(b) => b.get_id(db),
            HirNode::Expr(e) => e.get_id(db),
            HirNode::PathExpr(p) => p.get_id(db),
            HirNode::VariableAccess(v) => v.get_id(db),
            HirNode::Using(u) => u.get_id(db),
            HirNode::Param(p) => p.get_id(db),
            HirNode::InitExpr(i) => i.get_id(db),
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

impl<'db, T> HasComment<'db> for T where T: HirNodeInfo<'db> {}

