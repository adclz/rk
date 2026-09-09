use auto_lsp::{
    anyhow,
    lsp_types::{InlineValue, InlineValueParams},
};
use db::WorkspaceDataBase;
use ide_proto::handlers::inline_value::inline_values;

pub fn inline_value(
    db: &impl WorkspaceDataBase,
    params: InlineValueParams,
) -> anyhow::Result<Option<Vec<InlineValue>>> {
    let Some(file) = db.get_file(&params.text_document.uri) else {
        return Ok(None);
    };
    Ok(Some(inline_values(db, file, &params)))
}
