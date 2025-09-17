use std::ops::ControlFlow;

use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{CodeLens, CodeLensParams},
};
use hir::{hir_def::semantic_index::semantic_index, walk::WalkHir};

pub fn code_lens(
    db: &impl BaseDatabase,
    params: CodeLensParams,
) -> anyhow::Result<Option<Vec<CodeLens>>> {
    let uri = &params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];

    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node| {
        if let Some(code_lens) = node.as_proto().code_lens(db) {
            results.push(code_lens);
        }
        ControlFlow::Continue(())
    });

    Ok(Some(results))
}
