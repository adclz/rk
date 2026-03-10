use auto_lsp::{
    anyhow,
    lsp_types::{DocumentLink, DocumentLinkParams},
};
use db::WorkspaceDataBase;
use ide_proto::handlers::document_links::document_links as compute_document_links;

pub fn document_links(
    db: &impl WorkspaceDataBase,
    params: DocumentLinkParams,
) -> anyhow::Result<Option<Vec<DocumentLink>>> {
    let uri = &params.text_document.uri;
    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };
    let links = compute_document_links(db, file);
    if links.is_empty() {
        Ok(None)
    } else {
        Ok(Some(links))
    }
}
