use auto_lsp::{
    anyhow,
    lsp_types::{Hover, HoverParams},
};
use db::WorkspaceDataBase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::{
    handlers::HoverHandler,
    walk::{descendant_at, position_to_offset},
};

pub fn hover(db: &impl WorkspaceDataBase, params: HoverParams) -> anyhow::Result<Option<Hover>> {
    let _hover_span = tracing::info_span!("hover").entered();

    let uri = &params.text_document_position_params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let position =
        position_to_offset(db, file, params.text_document_position_params.position).ok_or_else(
            || {
                anyhow::format_err!(
                    "Invalid position, {:?}",
                    params.text_document_position_params.position
                )
            },
        )?;

    let sema = semantic_index(db, file);

    let symbol = descendant_at(db, file, position);
    match symbol.and_then(|s| s.hover(db, position)) {
        Some(hover) => Ok(Some(hover)),
        None => Ok(None),
    }
}
