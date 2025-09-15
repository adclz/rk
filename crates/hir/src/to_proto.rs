use auto_lsp::{
    core::{ast::AstNode, document_symbols_builder::DocumentSymbolsBuilder, span::Span},
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, GotoDefinitionResponse, Hover, InlayHint, request::GotoDeclarationResponse,
    },
};

use crate::hir_def::{scope::FileScopeId, semantic_index::semantic_index};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AstId(pub(crate) usize);

impl<T: AstNode> From<&T> for AstId {
    fn from(node: &T) -> Self {
        AstId(node.get_id())
    }
}

pub trait TypeInfo<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> &'static str;
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
            .unwrap_or_else(|| {
                panic!(
                    "Invalid ID {} when attempting to retrieve span",
                    self.get_id(db).0
                )
            })
            .get_span()
    }

    fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        self.get_name_id(db).map(|name_id| {
            semantic_index(db, self.get_scope_id(db).file(db))
                .ast
                .get(name_id.0)
                .unwrap_or_else(|| {
                    panic!(
                        "Invalid name ID {} when attempting to retrieve name span",
                        name_id.0
                    )
                })
                .get_span()
        })
    }

    // LSP

    fn document_symbols(&self, db: &'db dyn BaseDatabase, _builder: &mut DocumentSymbolsBuilder) {}

    fn completion(
        &'db self,
        _db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn inlay_hint(&'db self, _db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        None
    }

    fn hover(&'db self, _db: &'db dyn BaseDatabase, offset: Option<usize>) -> Option<Hover> {
        None
    }

    fn declaration(&'db self, _db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        None
    }

    fn definition(&'db self, _db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        None
    }
}
