use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{CodeActionOrCommand, CodeActionParams},
};
use hir::{check::diagnostics_for_file, def::semantic_index::semantic_index};

pub fn code_actions(
    db: &impl BaseDatabase,
    params: CodeActionParams,
) -> anyhow::Result<Option<Vec<CodeActionOrCommand>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];

    let ns = semantic_index(db, file);

    diagnostics_for_file(db, file)
        .iter()
        .for_each(|diagnostic| {
            if diagnostic.fixes().is_empty() {
                return;
            }
            if diagnostic.range().start <= range.end && diagnostic.range().end >= range.start {
                diagnostic.fixes().iter().for_each(|fix| {
                    results.push(CodeActionOrCommand::CodeAction(fix.clone()));
                });
            }
        });

    Ok(Some(results))
}
