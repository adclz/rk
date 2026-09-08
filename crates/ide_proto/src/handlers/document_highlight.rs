//! The occurrences of one symbol inside ONE file.
//!
//! This is `references` narrowed to the open document: the editor paints them
//! as the cursor moves, so it must not pay for a workspace scan. Every hit is
//! reported as `TEXT`; telling a read from a write would mean deciding what an
//! assignment target is, which is inference's answer and not worth a second
//! opinion here.

use auto_lsp::{
    default::db::file::File,
    lsp_types::{DocumentHighlight, DocumentHighlightKind},
};
use db::WorkspaceDataBase;
use hir::hir_def::hir_node::HirNode;

use crate::handlers::ReferencesHandler;

pub fn document_highlight<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
    file: File,
) -> Option<Vec<DocumentHighlight>> {
    let url = file.url(db);
    let highlights: Vec<DocumentHighlight> = node
        .locations(db)?
        .into_iter()
        .filter(|loc| loc.file.url(db) == url)
        .filter_map(|loc| {
            Some(DocumentHighlight {
                range: hir::denormalize(db, loc.file, &loc.span)?,
                kind: Some(DocumentHighlightKind::TEXT),
            })
        })
        .collect();

    (!highlights.is_empty()).then_some(highlights)
}
