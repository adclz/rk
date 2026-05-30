use auto_lsp::{
    anyhow,
    lsp_types::{Location, ReferenceParams},
};
use db::WorkspaceDataBase;
use ide_proto::{
    handlers::ReferencesHandler,
    walk::{descendant_at, position_to_offset},
};

pub fn references(
    db: &impl WorkspaceDataBase,
    params: ReferenceParams,
) -> anyhow::Result<Option<Vec<Location>>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let position = position_to_offset(db, file, params.text_document_position.position)
        .ok_or_else(|| {
            anyhow::format_err!("Invalid position, {:?}", params.text_document_position.position)
        })?;

    Ok(descendant_at(db, file, position).and_then(|s| s.references(db)))
}
