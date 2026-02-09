use auto_lsp::{
    anyhow,
    lsp_types::{CompletionParams, CompletionResponse},
};
use db::WorkspaceDataBase;
use ide_proto::walk::completion_descendant_at;

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
        Some(offset) => offset.saturating_sub(1),
        None => return Ok(None),
    };

    let target = match completion_descendant_at(db, file, offset) {
        Some(target) => target,
        None => {
            // no target node, show general completions (namespaces, pou snippets, etc)
            return Ok(Some(CompletionResponse::Array(vec![
                ide_proto::handlers::completions_utils::static_snippets::namespace(),
                ide_proto::handlers::completions_utils::static_snippets::using(),
                ide_proto::handlers::completions_utils::static_snippets::function(),
                ide_proto::handlers::completions_utils::static_snippets::function_block(),
                ide_proto::handlers::completions_utils::static_snippets::class(),
                ide_proto::handlers::completions_utils::static_snippets::interface(),
                ide_proto::handlers::completions_utils::static_snippets::type_(),
            ])));
        }
    };
    Ok(Some(CompletionResponse::Array(
        target
            .completion(db, offset, trigger_character, "".into())
            .unwrap_or_default(),
    )))
}
