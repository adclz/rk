use std::{ops::ControlFlow, panic::RefUnwindSafe};

use auto_lsp::{
    anyhow,
    lsp_types::{InlayHint, InlayHintParams},
};
use db::WorkspaceDataBase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::{walk::WalkHir};

pub fn inlay_hints<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    db: &Db,
    params: InlayHintParams,
) -> anyhow::Result<Option<Vec<InlayHint>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let mut results = vec![];

    let sema = semantic_index(db, file);
    let _ = sema.walk_hir(db, &mut |node| {
        if let Some(inlay_hint) = node.inlay_hint(db) {
            results.push(inlay_hint);
        }
        ControlFlow::Continue(())
    });

    Ok(Some(results))
}
