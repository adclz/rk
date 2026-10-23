// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::{
    anyhow,
    lsp_types::{CodeActionOrCommand, CodeActionParams},
};
use db::{WorkspaceDataBase, config_file::get_config};
use hir::check::check_file;
use hir::hir_ty::calls::CallGraph;

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

    let graph = CallGraph::of_files(db, [file]);
    let mut all = check_file(db, &graph, file);
    let linter_config = get_config(db).linter.clone().unwrap_or_default();
    linter::lint_file(db, file, &linter_config, &graph, &mut all);

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
