use auto_lsp::{anyhow, default::db::{BaseDatabase}, lsp_types::{CodeLens, CodeLensParams}};

pub fn code_lens(db: &impl BaseDatabase, params: CodeLensParams) -> anyhow::Result<Option<Vec<CodeLens>>> {
    let uri = &params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];
    Ok(Some(results))
}
