use auto_lsp::{
    core::{ast::AstNode, span::Span},
    default::db::BaseDatabase,
    lsp_types::{
        request::GotoDeclarationResponse, CompletionItem, GotoDefinitionResponse, Hover, InlayHint,
        SymbolKind,
    },
};

use crate::hir::{
    expressions::{expression::InitExpr, spec::Spec}, interned::namespace::SpannedNamespaceAccess, scope::FileScopeId, semantic_index::{semantic_index, SemanticIndex}
};

#[derive(bon::Builder, Debug, Clone)]
pub struct SymbolInfo<'a> {
    pub range: Span,
    pub name: String,
    pub name_range: Span,
    pub kind: Option<SymbolKind>,
    pub spec: Option<Spec<'a>>,
    pub init: Option<InitExpr<'a>>,
    pub implements: Option<Vec<SpannedNamespaceAccess>>,
    pub extends: Option<Extends<'a>>,
}

#[derive(Debug, Clone)]
pub enum Extends<'a> {
    Single(SpannedNamespaceAccess),
    Multiple(&'a Vec<SpannedNamespaceAccess>),
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
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> AstId;

    fn get_scope_id(&'db self, db: &'db dyn crate::BaseDatabase) -> FileScopeId;

    fn get_span(&'db self, db: &'db dyn crate::BaseDatabase) -> Span {
        let file = self.get_scope_id(db).file();
        semantic_index(db, file).ast
            .get(self.get_id(db).0)
            .expect("Invalid ID").get_span()
    }

    fn get_name_span(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<Span> {
        None
    }

    fn symbol_info(&'db self, _db: &'db dyn crate::BaseDatabase) -> Option<SymbolInfo<'db>> {
        None
    }

    // LSP

    fn completion(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn inlay_hint(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<InlayHint> {
        None
    }

    fn hover(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<Hover> {
        None
    }

    fn declaration(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<GotoDeclarationResponse> {
        None
    }

    fn definition(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<GotoDefinitionResponse> {
        None
    }
}

pub fn self_iter<'db>(s: &'db impl ToProto<'db>) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
    std::iter::once::<&'db dyn ToProto<'db>>(s)
}

pub trait IterToProto<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>>;

    #[tracing::instrument(skip(self, db, sema))]
    fn descendant_at(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<&'db dyn ToProto<'db>> {
        let mut best_match: Option<&'db dyn ToProto<'db>> = None;

        for node in self.iter(db, sema) {
            let range = node.get_span(db);
            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    let a = a.get_span(db);
                    if a.start_byte >= range.start_byte {
                        continue;
                    } else {
                        best_match = Some(node);
                    }
                } else {
                    best_match = Some(node);
                }
            }
        }
        best_match
    }

    #[tracing::instrument(skip(self, db, sema))]
    fn named_descendant_at(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<&'db dyn ToProto<'db>> {
        let mut best_match: Option<&'db dyn ToProto<'db>> = None;

        for node in self.iter(db, sema) {
            let range = match node.get_name_span(db) {
                Some(span) => span,
                None => continue,
            };

            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    let a = match a.get_name_span(db) {
                        Some(span) => span,
                        None => continue,
                    };

                    if a.start_byte >= range.start_byte {
                        continue;
                    } else {
                        best_match = Some(node);
                    }
                } else {
                    best_match = Some(node);
                }
            }
        }
        best_match
    }
}
