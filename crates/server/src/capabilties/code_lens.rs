use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{CodeLens, CodeLensParams},
};

pub fn code_lens(
    db: &impl BaseDatabase,
    params: CodeLensParams,
) -> anyhow::Result<Option<Vec<CodeLens>>> {
    let uri = &params.text_document.uri;

    let results = vec![];
    Ok(Some(results))
}
