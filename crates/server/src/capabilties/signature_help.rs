use auto_lsp::{
    anyhow,
    lsp_types::{SignatureHelp, SignatureHelpParams},
};
use db::WorkspaceDataBase;
use ide_proto::{handlers::signature_help::find_signature_help, walk::position_to_offset};

pub fn signature_help(
    db: &impl WorkspaceDataBase,
    params: SignatureHelpParams,
) -> anyhow::Result<Option<SignatureHelp>> {
    let uri = &params.text_document_position_params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let position =
        position_to_offset(db, file, params.text_document_position_params.position).ok_or_else(
            || {
                anyhow::format_err!(
                    "Invalid position, {:?}",
                    params.text_document_position_params.position
                )
            },
        )?;

    Ok(find_signature_help(db, file, position))
}
