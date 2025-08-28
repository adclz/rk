use std::ops::ControlFlow;

use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{InlayHint, InlayHintParams},
};
use hir::{def::semantic_index::semantic_index, walk::WalkHir};

pub fn inlay_hints(
    db: &impl BaseDatabase,
    params: InlayHintParams,
) -> anyhow::Result<Option<Vec<InlayHint>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];

    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node| {
        let span = node.get_span(db);
        if span.lsp().start.line < range.start.line || span.lsp().end.line > range.end.line {
            return ControlFlow::Break(());
        }
        if let Some(inlay_hint) = node.as_proto().inlay_hint(db, sema) {
            results.push(inlay_hint);
        }
        ControlFlow::Continue(())
    });

    Ok(Some(results))
}
