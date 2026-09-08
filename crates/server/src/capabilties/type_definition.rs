use auto_lsp::{
    anyhow,
    lsp_types::{GotoDefinitionResponse, request::GotoTypeDefinitionParams},
};
use db::WorkspaceDataBase;
use ide_proto::{
    handlers::type_definition::type_definition,
    walk::{descendant_at, position_to_offset},
};

pub fn go_to_type_definition(
    db: &impl WorkspaceDataBase,
    params: GotoTypeDefinitionParams,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let position = params.text_document_position_params;
    let Some(file) = db.get_file(&position.text_document.uri) else {
        return Ok(None);
    };
    let offset = position_to_offset(db, file, position.position)
        .ok_or_else(|| anyhow::format_err!("Invalid position, {:?}", position.position))?;
    Ok(descendant_at(db, file, offset).and_then(|node| type_definition(db, &node, offset)))
}
