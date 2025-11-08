#![allow(deprecated)]

use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{CompletionParams, CompletionResponse},
};
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;

pub fn completions(
    db: &impl BaseDatabase,
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

    Ok(semantic_index(db, file)
        .descendant_at(db, offset)
        .map(|s| {
            eprintln!("Getting completions for node: {s:?}, {offset}");
            CompletionResponse::Array(
                s.as_proto().completion(db, offset).unwrap_or_default(),
            )
        })
        .or_else(|| {
            Some(CompletionResponse::Array(vec![
                ide_proto::completions::static_snippets::namespace(),
                ide_proto::completions::static_snippets::function(),
                ide_proto::completions::static_snippets::function_block(),
                ide_proto::completions::static_snippets::class(),
                ide_proto::completions::static_snippets::interface(),
                ide_proto::completions::static_snippets::type_(),
            ]))
        }))
}
