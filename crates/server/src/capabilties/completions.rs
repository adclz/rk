#![allow(deprecated)]

use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{CompletionParams, CompletionResponse},
};
use ide_proto::completions::reparse::use_completion_marker;

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
        Some(offset) => offset,
        None => return Ok(None),
    };

    let _s = tracing::trace_span!("completions").entered();

    use_completion_marker(db, file, position, offset)
}
