use auto_lsp::{
    anyhow,
    core::dispatch,
    default::db::{tracked::get_ast, BaseDatabase, file::File},
    lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, SymbolKind},
};
use db::{diagnostics::cached_diagnostics, solver::namespace::namespaces_in_file, to_proto::IterToProto};

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

    let ns = namespaces_in_file(db, file).unwrap();
    for symbol in ns.iter(db) {
        let symbol = match symbol.symbol_info(db) {
            Some(symbol) => symbol,
            None => continue,
        };
        // Only process symbols that are inside the selected range
        if symbol.range.lsp().start >= range.start && symbol.range.lsp().end >= range.end {
            //if let Some(SymbolKind::NAMESPACE) = symbol.kind {
                results.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("Add internal to {}", symbol.name),
                    kind: Some(CodeActionKind::REFACTOR),
                    diagnostics: None,
                    is_preferred: None,
                    edit: None,
                    command: None,
                    data: None,
                    disabled: None,
                }));
            //}
        }
    }

    cached_diagnostics(db, file).iter().for_each(|diagnostic| {
        if diagnostic.fixes.is_empty() {
            return;
        }
        if diagnostic.diagnostic.range.start <= range.end
            && diagnostic.diagnostic.range.end >= range.start
        {
            diagnostic.fixes.iter().for_each(|fix| {
                results.push(CodeActionOrCommand::CodeAction(fix.clone()));
            });
        }
    });

    Ok(Some(results))
}
