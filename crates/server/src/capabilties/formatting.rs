use std::sync::LazyLock;

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

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    format(db, file)
}