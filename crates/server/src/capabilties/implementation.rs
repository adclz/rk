use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::request::{GotoImplementationParams, GotoImplementationResponse},
};
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;

pub fn go_to_implementation(
    db: &impl BaseDatabase,
    params: GotoImplementationParams,
) -> anyhow::Result<Option<GotoImplementationResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

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
