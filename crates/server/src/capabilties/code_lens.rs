use std::ops::ControlFlow;

use auto_lsp::{
    anyhow,
    lsp_types::{CodeLens, CodeLensParams},
};
use db::WorkspaceDataBase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::walk::WalkHir;

pub fn code_lens(
    db: &impl WorkspaceDataBase,
    params: CodeLensParams,
) -> anyhow::Result<Option<Vec<CodeLens>>> {
    let uri = &params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let mut results = vec![];

    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node| {
        if let Some(code_lens) = node.code_lens(db) {
            results.push(code_lens);
        }
        ControlFlow::Continue(())
    });

    Ok(Some(results))
}
