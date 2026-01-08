use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionParams, GotoDefinitionResponse},
};
use ide_proto::to_proto::{AsProtocol, hir_node::descendant_at};

pub fn go_to_definition(
    db: &impl BaseDatabase,
    params: GotoDefinitionParams,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
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
    Ok(descendant_at(db, file, position).and_then(|s| s.as_proto().definition(db)))
}
