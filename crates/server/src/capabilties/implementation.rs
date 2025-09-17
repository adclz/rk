use std::ops::ControlFlow;

use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, CodeLensParams,
        request::{GotoImplementationParams, GotoImplementationResponse},
    },
};
use hir::{hir_def::semantic_index::semantic_index, walk::WalkHir};

pub fn go_to_implementation(
    db: &impl BaseDatabase,
    params: GotoImplementationParams,
) -> anyhow::Result<Option<GotoImplementationResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let document = file.document(db);

    let position = document
        .offset_at(params.text_document_position_params.position)
        .ok_or_else(|| {
            anyhow::format_err!(
                "Invalid position, {:?}",
                params.text_document_position_params.position
            )
        })?;
    Ok(semantic_index(db, file)
        .descendant_at(db, position)
        .and_then(|s| s.as_proto().implementation(db)))
}
