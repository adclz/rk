use auto_lsp::{
    anyhow,
    lsp_types::{Location, ReferenceParams},
};
use db::WorkspaceDataBase;
use ide_proto::walk::descendant_at;

pub fn references(
    db: &impl WorkspaceDataBase,
    params: ReferenceParams,
) -> anyhow::Result<Option<Vec<Location>>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let document = file.document(db);

    let position = document
        .offset_at(params.text_document_position.position)
        .ok_or_else(|| {
            anyhow::format_err!(
                "Invalid position, {:?}",
                params.text_document_position.position
            )
        })?;

    Ok(descendant_at(db, file, position)
        .and_then(|s| s.references(db))
        .map(|refs| refs.iter().map(|r| r.to_location(db)).collect()))
}
