use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel,
        MarkedString,
    },
};
use hir::{
    hir_def::{namespace::NamespaceDecl, semantic_index::get_scope}, HirNodeInfo
};

use crate::{completions, to_proto::ToProtocol};

impl<'db> ToProtocol<'db> for NamespaceDecl<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        let ns = self.path(db).to_string(db);
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(
                format!(
                    r#"
```iecst
NAMESPACE {ns}
```
                    "#
                )
                .to_string(),
            )),
            range: None,
        })
    }

    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        self.namespaces(db)
            .iter()
            .for_each(|ns| ns.document_symbols(db, &mut nested_builder));

        self.pous(db)
            .iter()
            .for_each(|pou| pou.document_symbols(db, &mut nested_builder));

        let name = self.path(db).to_string(db);
        let name = match name.len() {
            0 => "?".into(),
            _ => name
        };

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some("NAMESPACE".to_string()),
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
            label: InlayHintLabel::String(format!("NAMESPACE {}", self.path(db).to_string(db))),
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
        let scope = get_scope(db, self.scope_id(db));

        // Don't provide completions between the namespace keyword and the namespace name
        /*if self.get_name_span(db)?.end_byte > offset {
            if !self.visibility == Visibility::PUBLIC {
                return Some(vec![CompletionItem::new_simple(
                    "INTERNAL".into(),
                    "internal".into(),
                )]);
            } else {
                return None;
            }
        }*/

        let mut completions = vec![
            completions::static_snippets::namespace(),
            completions::static_snippets::function(),
            completions::static_snippets::function_block(),
            completions::static_snippets::type_(),
            completions::static_snippets::class(),
            completions::static_snippets::interface(),
        ];
        // Using directives can only be added before any POU declarations
        if let Some(pou) = self.pous(db).first() {
            if pou.get_span(db).end_byte >= offset {
                completions.push(completions::static_snippets::using());
            }
        } else {
            completions.push(completions::static_snippets::using());
        }
        Some(completions)
    }
}
