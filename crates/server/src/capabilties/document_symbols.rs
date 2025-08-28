#![allow(deprecated)]

use auto_lsp::{
    anyhow,
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{DocumentSymbolParams, DocumentSymbolResponse},
};

pub fn document_symbols(
    db: &impl BaseDatabase,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let builder = DocumentSymbolsBuilder::default();

    // Build hierarchy by finding parent-child relationships

    Ok(Some(DocumentSymbolResponse::Nested(builder.finalize())))
}
