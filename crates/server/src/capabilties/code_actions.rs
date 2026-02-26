use auto_lsp::{
    anyhow,
    lsp_types::{CodeActionOrCommand, CodeActionParams},
};
use db::{WorkspaceDataBase, config_file::get_config};
use hir::check::diagnostics_for_file;

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

    let mut all = diagnostics_for_file(db, file).as_ref().clone();
    if let Some(ref linter_config) = get_config(db).linter {
        linter::lint_file(db, file, linter_config, &mut all);
    }

    let mut results = vec![];

    all.iter().for_each(|diagnostic| {
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
