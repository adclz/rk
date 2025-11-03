use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverParams},
};
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;

pub fn hover(db: &impl BaseDatabase, params: HoverParams) -> anyhow::Result<Option<Hover>> {
    let _hover_span = tracing::info_span!("hover").entered();

    let uri = &params.text_document_position_params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let document = file.document(db);

    let position = document
        .offset_at(params.text_document_position_params.position)
        .ok_or_else(|| {
            anyhow::format_err!(
                "Invalid position, {:?}",
                params.text_document_position_params.position
            )
        })?;

    let sema = semantic_index(db, file);

    let symbol = sema.descendant_at(db, position);
    match symbol.and_then(|s| s.as_proto().hover(db, position)) {
        Some(hover) => {
            //eprintln!("Hover generated: {:?}", hover);
            Ok(Some(hover))
        },
        None => Ok(None),
    }
}
