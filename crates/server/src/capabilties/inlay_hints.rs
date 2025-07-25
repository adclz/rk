use auto_lsp::{anyhow, default::db::{BaseDatabase}, lsp_types::{InlayHint, InlayHintParams}};
use db::{solver::namespace::namespaces_in_file, to_proto::{IterToProto}};

pub fn inlay_hints(db: &impl BaseDatabase, params: InlayHintParams) -> anyhow::Result<Option<Vec<InlayHint>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];

    let ns = match namespaces_in_file(db, file) {
        Some(ns) => ns,
        None => return Ok(None),
    };
    ns.iter(db).for_each(|symbol| {
        let span = symbol.get_span(db);
        if span.lsp().start.line < range.start.line ||
           span.lsp().end.line > range.end.line {
            return;
        }

        if let Some(inlay_hint) = symbol.inlay_hint(db) {
            results.push(inlay_hint);
        }
    });

    Ok(Some(results))
}