#![allow(deprecated)]

use auto_lsp::{
    anyhow,
    core::{document_symbols_builder::DocumentSymbolsBuilder, span::Span},
    default::db::BaseDatabase,
    lsp_types::{DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse},
};
use hir::{
    def::semantic_index::semantic_index,
    to_proto::{SymbolInfo},
};

pub fn document_symbols(
    db: &impl BaseDatabase,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut builder = DocumentSymbolsBuilder::default();

    // Build hierarchy by finding parent-child relationships

    Ok(Some(DocumentSymbolResponse::Nested(builder.finalize())))
}
