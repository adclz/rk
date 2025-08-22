#![allow(deprecated)]

use auto_lsp::{
    anyhow,
    core::{document_symbols_builder::DocumentSymbolsBuilder, span::Span},
    default::db::BaseDatabase,
    lsp_types::{DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse},
};
use hir::{
    def::semantic_index::semantic_index,
    to_proto::{IterToProto, SymbolInfo},
};

/// Helper function to check if one range is inside another
fn is_range_inside(inner: &Span, outer: &Span) -> bool {
    let inner_lsp = inner.lsp();
    let outer_lsp = outer.lsp();

    // Check if inner range is completely contained within outer range
    (outer_lsp.start.line < inner_lsp.start.line
        || (outer_lsp.start.line == inner_lsp.start.line
            && outer_lsp.start.character <= inner_lsp.start.character))
        && (inner_lsp.end.line < outer_lsp.end.line
            || (inner_lsp.end.line == outer_lsp.end.line
                && inner_lsp.end.character <= outer_lsp.end.character))
}

/// Recursively build children for a parent symbol
fn build_children(
    parent: &SymbolInfo,
    all_symbols: &[(SymbolInfo, auto_lsp::lsp_types::SymbolKind)],
) -> Option<Vec<DocumentSymbol>> {
    let children: Vec<_> = all_symbols
        .iter()
        .filter(|(symbol, _)| {
            // Find symbols that are direct children of parent
            symbol.name != parent.name &&
            is_range_inside(&symbol.range, &parent.range) &&
            // Make sure it's not a grandchild (not inside another child)
            !all_symbols.iter().any(|(other, _)| {
                other.name != symbol.name &&
                other.name != parent.name &&
                is_range_inside(&symbol.range, &other.range) &&
                is_range_inside(&other.range, &parent.range)
            })
        })
        .map(|(symbol, kind)| {
            let mut child = DocumentSymbol {
                name: symbol.name.clone(),
                detail: None,
                kind: *kind,
                selection_range: symbol.name_range.lsp(),
                deprecated: None,
                tags: None,
                children: None,
                range: symbol.range.lsp(),
            };
            // Recursively build children for this child
            child.children = build_children(symbol, all_symbols);
            child
        })
        .collect();

    if children.is_empty() {
        None
    } else {
        Some(children)
    }
}

pub fn document_symbols(
    db: &impl BaseDatabase,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut builder = DocumentSymbolsBuilder::default();
    let ns = semantic_index(db, file);

    // Collect all symbols first
    let symbols: Vec<_> = ns
        .iter(db, ns)
        .filter_map(|symbol| symbol.symbol_info(db))
        .filter(|symbol| !symbol.name.is_empty())
        .filter_map(|symbol| symbol.kind.map(|kind| (symbol, kind)))
        .collect();

    // Build hierarchy by finding parent-child relationships
    let mut root_symbols = Vec::new();

    for (symbol, kind) in &symbols {
        // Check if this symbol is inside any other symbol
        let parent = symbols.iter().find(|(other_symbol, _)| {
            // Check if symbol is inside other_symbol and they're not the same
            other_symbol.name != symbol.name && is_range_inside(&symbol.range, &other_symbol.range)
        });

        let document_symbol = DocumentSymbol {
            name: symbol.name.clone(),
            detail: None,
            kind: *kind,
            selection_range: symbol.name_range.lsp(),
            deprecated: None,
            tags: None,
            children: None,
            range: symbol.range.lsp(),
        };

        if parent.is_none() {
            // This is a root symbol
            root_symbols.push((document_symbol, symbol));
        }
    }

    // Now build the children for each root symbol
    for (mut root_symbol, root_symbol_info) in root_symbols {
        root_symbol.children = build_children(root_symbol_info, &symbols);
        builder.push_symbol(root_symbol);
    }

    Ok(Some(DocumentSymbolResponse::Nested(builder.finalize())))
}
