use auto_lsp::{
    anyhow,
    lsp_types::request::{GotoImplementationParams, GotoImplementationResponse},
};
use db::WorkspaceDataBase;
use ide_proto::{
    handlers::ImplementationHandler,
    walk::{descendant_at, position_to_offset},
};

pub fn go_to_implementation(
    db: &impl WorkspaceDataBase,
    params: GotoImplementationParams,
) -> anyhow::Result<Option<GotoImplementationResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let position = position_to_offset(db, file, params.text_document_position_params.position)
        .ok_or_else(|| {
            anyhow::format_err!(
                "Invalid position, {:?}",
                params.text_document_position_params.position
            )
        })?;
    Ok(descendant_at(db, file, position).and_then(|s| s.implementation(db)))
}
