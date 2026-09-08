use auto_lsp::{
    anyhow,
    lsp_types::{DocumentHighlight, DocumentHighlightParams},
};
use db::WorkspaceDataBase;
use ide_proto::{
    handlers::document_highlight::document_highlight,
    walk::{descendant_at, position_to_offset},
};

pub fn highlights(
    db: &impl WorkspaceDataBase,
    params: DocumentHighlightParams,
) -> anyhow::Result<Option<Vec<DocumentHighlight>>> {
    let position = params.text_document_position_params;
    let Some(file) = db.get_file(&position.text_document.uri) else {
        return Ok(None);
    };
    let offset = position_to_offset(db, file, position.position)
        .ok_or_else(|| anyhow::format_err!("Invalid position, {:?}", position.position))?;
    Ok(descendant_at(db, file, offset).and_then(|node| document_highlight(db, &node, file)))
}
