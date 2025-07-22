use auto_lsp::{anyhow, default::db::{BaseDatabase}, lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams}};
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
    ns.namespaces(db).iter().for_each(|symbol| {
        let span = symbol.span(db);
        if span.lsp().start.line < range.start.line ||
           span.lsp().end.line > range.end.line {
            return;
        }
        results.push(InlayHint {
            label: InlayHintLabel::String(format!("{:?}", symbol.parent(db))),
            position: span.lsp().end,
            kind: Some(InlayHintKind::TYPE),   
            text_edits: None,
            padding_left: Some(true),
            padding_right: None, 
            data: None,
            tooltip: None,
         });
    });

    Ok(Some(results))
}