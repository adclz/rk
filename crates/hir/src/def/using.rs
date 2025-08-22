use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, MarkupContent, MarkupKind},
};

use crate::{
    def::{interned::namespace::NamespacePath, scope::FileScopeId, semantic_index::SemanticIndex},
    to_proto::{AstId, IterToProto, ToProto, self_iter},
};

#[salsa::tracked(debug)]
pub struct Using<'db> {
    pub path: NamespacePath,

    pub id: AstId,

    pub scope_id: FileScopeId,
}

impl<'db> ToProto<'db> for Using<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }

    fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        Some(self.get_span(db))
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("Using namespace `{}`", self.path(db).to_string(db)),
            }),
            range: Some(self.get_span(db).lsp()),
        })
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
        /* let fragments = self.path(db).fragments(db);
        let mut marker_index = None;

        for (i, fragment) in fragments.iter().enumerate() {
            if fragment.ident.text(db).contains(COMPLETION_MARKER) {
                marker_index = Some(i);
                break;
            }
        }

        let marker_index = marker_index?;

        let mut seen = FxHashSet::default();

        // Case 1: marker is in the first fragment -> we can only prefix-match from root
        if marker_index == 0 {
            let prefix = fragments[0].ident.text(db).replace(COMPLETION_MARKER, "");
            return Some(
                starts_with(db, Ident::new(db, prefix))
                    .iter()
                    .filter_map(|ns| ns.path(db).fragments(db).get(0))
                    .filter(|ident| seen.insert(*ident))
                    .map(|ident| {
                        CompletionItem::new_simple(ident.ident.text(db), ident.ident.text(db))
                    })
                    .collect(),
            );
        }

        // Case 2: marker is in a deeper fragment -> walk through layers with exact match
        let mut matching = starts(db, fragments[0].ident).to_vec();

        for i in 1..marker_index {
            matching = matching
                .into_iter()
                .filter(|ns| {
                    ns.path(db)
                        .fragments(db)
                        .get(i)
                        .map_or(false, |frag| frag == &fragments[i])
                })
                .collect();
        }

        // Now suggest completions for the fragment at `marker_index`
        let completions = matching
            .iter()
            .filter_map(|ns| ns.path(db).fragments(db).get(marker_index))
            .filter(|ident| seen.insert(*ident))
            .map(|ident| CompletionItem::new_simple(ident.ident.text(db), ident.ident.text(db)))
            .collect();

        Some(completions)*/
    }
}

impl<'db> IterToProto<'db> for Using<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        _sema: &SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self_iter(self).chain(self.path(db).fragments(db).iter().map(move |f| f as _))
    }
}
