use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionParams, GotoDefinitionResponse},
};
use db::hir::semantic_index::semantic_index;
use db::to_proto::IterToProto;

pub fn go_to_definition(db: &impl BaseDatabase, params: GotoDefinitionParams) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

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

    let symbol = sema
        .named_descendant_at(db, &sema, position)
        .or_else(|| sema.descendant_at(db, &sema, position));

    match symbol.and_then(|s| s.definition(db, &sema)) {
        Some(def) => Ok(Some(def)),
        None => Ok(None),
    }
}
