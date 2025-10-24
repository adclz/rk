use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{hir_def::{interned::identifier::Ident, namespace::NamespaceDecl, semantic_index::semantic_index, using::Using}, HirNodeInfo};
use rustc_hash::FxHashSet;

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Using<'db> {
    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let fragments = self.path(db).fragments(db);
        let mut marker_index = None;

        for (i, fragment) in fragments.iter().enumerate() {
            let frag_span = fragment.get_span(db);
            if frag_span.start_byte <= _offset{
                marker_index = Some(i);
                break;
            }
        }

        let marker_index = marker_index?;

        let mut seen = FxHashSet::default();

        // Case 1: marker is in the first fragment -> we can only prefix-match from root
        if marker_index == 0 {
            let prefix = fragments[0].ident.text(db);
            return Some(
                starts_with(db, Ident::new(db, prefix))
                    .iter()
                    .filter_map(|ns| ns.path(db).fragments(db).get(0))
                    .filter(|ident| seen.insert(*ident))
                    .map(|ident| {
                        CompletionItem::new_simple(ident.text(db).to_string(), ident.text(db).to_string())
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
            .map(|ident| CompletionItem::new_simple(ident.text(db).to_string(), ident.text(db).to_string()))
            .collect();

        Some(completions)
    }
}


fn starts_with<'db>(
    db: &'db dyn BaseDatabase,
    prefix: Ident,
) -> Vec<NamespaceDecl<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| {
            semantic_index(db, *file)
                .namespaces
                .iter()
                .filter(|ns| ns.path(db).fragments(db).get(0).map_or(false, |frag| {
                    frag.text(db).starts_with(prefix.text(db).as_str())
                }))
                .copied()
        })
        .collect()
}

fn starts<'db>(
    db: &'db dyn BaseDatabase,
    ident: Ident,
) -> Vec<NamespaceDecl<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| {
            semantic_index(db, *file)
                .namespaces
                .iter()
                .filter(|ns| ns.path(db).fragments(db).get(0).map_or(false, |frag| {
                    frag == &ident
                }))
                .copied()
        })
        .collect()
}