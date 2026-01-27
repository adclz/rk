use auto_lsp::{
    anyhow,
    lsp_types::{CompletionParams, CompletionResponse},
};
use db::WorkspaceDataBase;
use ide_proto::{walk::descendant_at};

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
    let offset = match doc.offset_at(position) {
        Some(offset) => match params.context.unwrap().trigger_character {
            Some(str) if str == "." => offset.saturating_sub(1),
            _ => offset,
        },
        None => return Ok(None),
    };

    Ok(descendant_at(db, file, offset)
        .map(|s| CompletionResponse::Array(s.completion(db, offset).unwrap_or_default()))
        .or_else(|| {
            Some(CompletionResponse::Array(vec![
                ide_proto::handlers::completion::static_snippets::namespace(),
                ide_proto::handlers::completion::static_snippets::function(),
                ide_proto::handlers::completion::static_snippets::function_block(),
                ide_proto::handlers::completion::static_snippets::class(),
                ide_proto::handlers::completion::static_snippets::interface(),
                ide_proto::handlers::completion::static_snippets::type_(),
            ]))
        }))
}
