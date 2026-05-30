use auto_lsp::{
    anyhow,
    lsp_types::{RenameParams, WorkspaceEdit},
};
use db::WorkspaceDataBase;
use ide_proto::{
    handlers::RenameHandler,
    walk::{descendant_at, position_to_offset},
};

pub fn rename(
    db: &impl WorkspaceDataBase,
    params: RenameParams,
) -> anyhow::Result<Option<WorkspaceEdit>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let position = position_to_offset(db, file, params.text_document_position.position)
        .ok_or_else(|| {
            anyhow::format_err!(
                "Invalid position, {:?}",
                params.text_document_position.position
            )
        })?;

    Ok(descendant_at(db, file, position).and_then(|s| s.rename(db, &params.new_name)))
}
