use auto_lsp::{
    anyhow,
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{DocumentSymbolParams, DocumentSymbolResponse},
};
use db::WorkspaceDataBase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::to_proto::ToProtocol;

pub fn document_symbols(
    db: &impl WorkspaceDataBase,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;

    let file = match db.get_file(&uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(db, file);

    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(db, &mut builder));
    sema.namespaces
        .iter()
        .for_each(|ns| ns.document_symbols(db, &mut builder));

    Ok(Some(DocumentSymbolResponse::Nested(builder.finalize())))
}
