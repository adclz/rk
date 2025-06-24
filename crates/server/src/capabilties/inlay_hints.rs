use auto_lsp::{anyhow, default::db::{BaseDatabase}, lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams}};
use db::{solver::{namespaces_in_file}, to_proto::IterToProto};

pub fn inlay_hints(db: &impl BaseDatabase, params: InlayHintParams) -> anyhow::Result<Option<Vec<InlayHint>>> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut results = vec![];

    let ns = namespaces_in_file(db, file).unwrap();
    ns.iter(db).for_each(|symbol| {
        if symbol.range.as_lsp().start.line < range.start.line ||
           symbol.range.as_lsp().end.line > range.end.line {
            return;
        }
        results.push(InlayHint {
            label: InlayHintLabel::String(format!("{} {}", match symbol.kind {
                Some(auto_lsp::lsp_types::SymbolKind::NAMESPACE) => "namespace",
                Some(auto_lsp::lsp_types::SymbolKind::FUNCTION) => "function",
                Some(auto_lsp::lsp_types::SymbolKind::CLASS) => "class",
                Some(auto_lsp::lsp_types::SymbolKind::INTERFACE) => "interface",
                Some(auto_lsp::lsp_types::SymbolKind::TYPE_PARAMETER) => "type",
                Some(auto_lsp::lsp_types::SymbolKind::VARIABLE) => "variable",
                _ => "unknown",
            }, symbol.name)),
            position: symbol.range.as_lsp().end,
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