use auto_lsp::{
    core::{ast::AstNode, span::Span},
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, GotoDefinitionResponse, Hover, InlayHint, SymbolKind,
        request::GotoDeclarationResponse,
    },
};

use crate::def::{
        expressions::{expression::InitExpr, spec::Spec},
        interned::namespace::SpanNamespaceAccess,
        scope::FileScopeId,
        semantic_index::{SemanticIndex, semantic_index},
    };

#[derive(bon::Builder, Debug, Clone)]
pub struct SymbolInfo<'a> {
    pub range: Span,
    pub name: String,
    pub name_range: Span,
    pub kind: Option<SymbolKind>,
    pub spec: Option<Spec<'a>>,
    pub init: Option<InitExpr<'a>>,
    pub implements: Option<Vec<SpanNamespaceAccess<'a>>>,
    pub extends: Option<Extends<'a>>,
}

#[derive(Debug, Clone)]
pub enum Extends<'a> {
    Single(SpanNamespaceAccess<'a>),
    Multiple(&'a Vec<SpanNamespaceAccess<'a>>),
}

impl SymbolInfo<'_> {
    pub fn kind_to_string(&self) -> &'static str {
        match self.kind {
            Some(SymbolKind::VARIABLE) => "var",
            Some(SymbolKind::FUNCTION) => "function",
            Some(SymbolKind::CLASS) => "class",
            Some(SymbolKind::INTERFACE) => "interface",
            Some(SymbolKind::TYPE_PARAMETER) => "type",
            Some(SymbolKind::NAMESPACE) => "namespace",
            Some(SymbolKind::METHOD) => "method",
            _ => "unknown",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AstId(pub(crate) usize);

impl<T: AstNode> From<&T> for AstId {
    fn from(node: &T) -> Self {
        AstId(node.get_id())
    }
}

pub trait ToProto<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId;

    fn get_name_id(&'db self, _db: &'db dyn BaseDatabase) -> Option<AstId> {
        None
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db>;

    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> Span {
        semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_id(db).0)
            .unwrap_or_else(|| panic!("Invalid ID {} when attempting to retrieve span",
                self.get_id(db).0))
            .get_span()
    }

    fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        self.get_name_id(db).map(|name_id| {
            semantic_index(db, self.get_scope_id(db).file(db))
                .ast
                .get(name_id.0)
                .unwrap_or_else(|| panic!("Invalid name ID {} when attempting to retrieve name span",
                    name_id.0))
                .get_span()
        })
    }

    fn symbol_info(&'db self, _db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        None
    }

    // LSP

    fn completion(
        &'db self,
        _db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn inlay_hint(
        &'db self,
        _db: &'db dyn BaseDatabase,
    ) -> Option<InlayHint> {
        None
    }

    fn hover(
        &'db self,
        _db: &'db dyn BaseDatabase,
    ) -> Option<Hover> {
        None
    }

    fn declaration(
        &'db self,
        _db: &'db dyn BaseDatabase,
    ) -> Option<GotoDeclarationResponse> {
        None
    }

    fn definition(
        &'db self,
        _db: &'db dyn BaseDatabase,
    ) -> Option<GotoDefinitionResponse> {
        None
    }
}
