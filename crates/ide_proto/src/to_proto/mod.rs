use auto_lsp::{
    core::{
        document_symbols_builder::DocumentSymbolsBuilder,
        semantic_tokens_builder::SemanticTokensBuilder,
    },
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    },
};
use db::WorkspaceDataBase;
use hir::HirNodeInfo;

use crate::{comment_index::comment_index, to_proto::hir_node::HirNode};

pub mod hir_node;
pub mod walk;

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

pub trait ToProtocol<'db>: HirNodeInfo<'db> {
    fn document_symbols(&self, _db: &'db dyn WorkspaceDataBase, _builder: &mut DocumentSymbolsBuilder) {}

    fn completion(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn code_lens(&self, _db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        None
    }

    fn implementation(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<GotoImplementationResponse> {
        None
    }

    fn inlay_hint(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        None
    }

    fn hover(&'db self, _db: &'db dyn WorkspaceDataBase, _offset: usize) -> Option<Hover> {
        None
    }

    fn declaration(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        None
    }

    fn definition(&'db self, _db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        None
    }

    fn semantic_tokens(&'db self, _db: &'db dyn WorkspaceDataBase, _builder: &mut SemanticTokensBuilder) {
    }
}

pub trait AsProtocol<'db> {
    fn as_proto(&'db self) -> &'db dyn ToProtocol<'db>;
}

impl<'db> AsProtocol<'db> for HirNode<'db> {
    fn as_proto(&'db self) -> &'db dyn ToProtocol<'db> {
        match self {
            HirNode::Namespace(ns) => ns,
            HirNode::SpanNamespaceAccess(s) => s,
            HirNode::PouDecl(p) => p,
            HirNode::VariableDecl(v) => v,
            HirNode::StructElement(s) => s,
            HirNode::Spec(s) => s,
            HirNode::MethodRef(m) => m,
            HirNode::Using(u) => u,
            HirNode::BeginPathExpr(b) => b,
            HirNode::Expr(e) => e,
            HirNode::PathExpr(p) => p,
            HirNode::VariableAccess(v) => v,
            HirNode::InitExprWithType(e) => e,
            HirNode::Param(p) => p,
        }
    }
}
