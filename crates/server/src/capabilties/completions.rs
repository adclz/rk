use auto_lsp::{
    anyhow,
    lsp_types::{CompletionParams, CompletionResponse},
};
use db::WorkspaceDataBase;
use ide_proto::handlers::completions::complete;

pub fn completions(
    db: &impl WorkspaceDataBase,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let doc = file.document(db);

    let position = params.text_document_position.position;
    let trigger_character = params
        .context
        .as_ref()
        .and_then(|ctx| ctx.trigger_character.clone());

    let offset = match doc.offset_at(position) {
        Some(offset) => offset,
        None => return Ok(None),
    };

    let items = complete(db, file, offset, trigger_character);
    Ok(Some(CompletionResponse::Array(items)))
}
