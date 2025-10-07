use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{DocumentFormattingParams, TextEdit},
};
use formatter::format;

pub fn formatting(
    db: &impl BaseDatabase,
    params: DocumentFormattingParams,
) -> anyhow::Result<Option<Vec<TextEdit>>> {
    let uri = &params.text_document.uri;

   let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    format(db, file)
}
