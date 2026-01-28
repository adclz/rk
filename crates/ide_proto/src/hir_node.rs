use auto_lsp::{core::{document_symbols_builder::DocumentSymbolsBuilder, semantic_tokens_builder::SemanticTokensBuilder}, lsp_types::{CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint, request::{GotoDeclarationResponse, GotoImplementationResponse}}};
use db::WorkspaceDataBase;
use hir::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, ParamAssignKind, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        }, interned::namespace::SpanNamespaceAccess, namespace::NamespaceDecl, pous::{pou::Pou, variable::VariableDecl}, scope::ScopeId, using::Using
    },
    hir_ty::signature::inheritance::MethodRef,
};

use crate::{comment_index::comment_index, handlers::{CodeLensHandler, DeclarationHandler, DefinitionHandler, DocumentSymbolsHandler, HoverHandler, InlayHintHandler, SemanticTokensHandler}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    NamespaceAccess(SpanNamespaceAccess<'db>),
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
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        match self {
            HirNode::Namespace(n) => n.document_symbols(db, builder),
            HirNode::PouDecl(p) => p.document_symbols(db, builder),
            HirNode::VariableDecl(v) => v.document_symbols(db, builder),
            HirNode::MethodRef(m) => m.document_symbols(db, builder),
            _ => ()
        }
    }

    pub fn completion(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    pub fn code_lens(&self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        match self {
            HirNode::PouDecl(pou) => pou.code_lens(db),
            _ => None,
        }
    }

    pub fn implementation(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        None
    }

    pub fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        match self {
            HirNode::Namespace(n) => n.inlay_hint(db),
            HirNode::PouDecl(p) => p.inlay_hint(db),
            HirNode::Param(p) => p.inlay_hint(db),
            HirNode::InitExpr(init_expr) => init_expr.inlay_hint(db),
            _ => None,
        }
    }

    pub fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        match self {
            HirNode::Namespace(n) => n.hover(db, offset),
            HirNode::PouDecl(p) => p.hover(db, offset),
            HirNode::VariableDecl(v) => v.hover(db, offset),
            HirNode::Spec(s) => s.hover(db, offset),
            HirNode::MethodRef(m) => m.hover(db, offset),
            HirNode::StructElement(st) => st.hover(db, offset),
            HirNode::BeginPathExpr(b) => b.hover(db, offset),
            HirNode::PathExpr(p) => p.hover(db, offset),
            HirNode::VariableAccess(v) => v.hover(db, offset),
            HirNode::Expr(e) => e.hover(db, offset),
            HirNode::Using(u) => u.hover(db, offset),
            HirNode::Param(p) => p.hover(db, offset),
            _ => None,
        }
    }

    pub fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        match self {
            HirNode::VariableDecl(v) => v.declaration(db),
            HirNode::StructElement(s) => s.declaration(db),
            HirNode::Spec(s) => s.declaration(db),
            HirNode::BeginPathExpr(b) => b.declaration(db),
            HirNode::PathExpr(p) => p.declaration(db),
            HirNode::VariableAccess(v) => v.declaration(db),
            HirNode::Expr(e) => e.declaration(db),
            HirNode::Param(p) => p.declaration(db),
            _ => None,
        }
    }

    pub fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        match self {
            HirNode::PouDecl(pou) => pou.definition(db),
            HirNode::VariableDecl(v) => v.definition(db),
            HirNode::StructElement(s) => s.definition(db),
            HirNode::Spec(s) => s.definition(db),
            HirNode::BeginPathExpr(b) => b.definition(db),
            HirNode::PathExpr(p) => p.definition(db),
            HirNode::VariableAccess(v) => v.definition(db),
            HirNode::Expr(e) => e.definition(db),
            HirNode::Param(p) => p.definition(db),
            _ => None,
        }
    }

    pub fn semantic_tokens(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _builder: &mut SemanticTokensBuilder,
    ) {
        match self {
            HirNode::VariableDecl(v) => v.semantic_tokens(_db, _builder),
            _ => {}
        }
    }
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
            HirNode::NamespaceAccess(n) => n.get_id(db),
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