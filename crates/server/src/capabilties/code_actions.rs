use auto_lsp::{
    anyhow,
    lsp_types::{CodeActionOrCommand, CodeActionParams},
};
use db::WorkspaceDataBase;
use hir::{check::diagnostics_for_file, hir_def::semantic_index::semantic_index};

pub fn code_actions(
    db: &impl WorkspaceDataBase,
    params: CodeActionParams,
) -> anyhow::Result<Option<Vec<CodeActionOrCommand>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

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
