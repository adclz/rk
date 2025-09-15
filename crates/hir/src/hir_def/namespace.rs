use crate::completions;
use auto_lsp::core::document_symbols_builder::DocumentSymbolsBuilder;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{
    CompletionItem, InlayHint, InlayHintKind, InlayHintLabel
};

use crate::hir_def::interned::namespace::NamespacePath;
use crate::hir_def::pous::pou::PouDecl;
use crate::hir_def::scope::{FileScopeId, Visibility};
use crate::hir_def::semantic_index::semantic_index;
use crate::to_proto::{AstId, ToProto};

#[salsa::tracked(debug)]
pub struct NamespaceDecl<'db> {
    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub pous: Vec<PouDecl<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> ToProto<'db> for NamespaceDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> crate::to_proto::AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        self.pous(db)
            .iter()
            .for_each(|pou| pou.document_symbols(db, &mut nested_builder));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name: self.path(db).to_string(db),
            detail: Some("namespace".to_string()),
            kind: auto_lsp::lsp_types::SymbolKind::NAMESPACE, // Namespace
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).unwrap().lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }

    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<auto_lsp::lsp_types::InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!("namespace {}", self.path(db).to_string(db))),
            position: self.get_span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let sema = semantic_index(db, self.scope_id(db).file(db));
        let scope = sema.get_scope(db, self.scope_id(db));

        // Don't provide completions between the namespace keyword and the namespace name
        if self.get_name_span(db)?.end_byte > offset {
            if !scope.visibility == Visibility::PUBLIC {
                return Some(vec![CompletionItem::new_simple(
                    "INTERNAL".into(),
                    "internal".into(),
                )]);
            } else {
                return None;
            }
        }

        let mut completions = vec![
            completions::snippets::namespace(),
            completions::snippets::function(),
            completions::snippets::function_block(),
            completions::snippets::type_(),
            completions::snippets::class(),
            completions::snippets::interface(),
        ];
        // Using directives can only be added before any POU declarations
        if let Some(pou) = self.pous(db).first() {
            if pou.get_span(db).end_byte >= offset {
                completions.push(completions::snippets::using());
            }
        } else {
            completions.push(completions::snippets::using());
        }
        Some(completions)
    }
}
