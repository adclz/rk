#![allow(deprecated)]

use std::sync::Arc;

use auto_lsp::{
    anyhow,
    default::db::{file::File, BaseDatabase},
    lsp_types::{self, CompletionParams, CompletionResponse},
};
use db::{hir::COMPLETION_MARKER, hir::semantic_index::semantic_index, to_proto::{IterToProto}};

pub fn completions(
    db: &impl BaseDatabase,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let doc = file.document(db);

    let position = params.text_document_position.position;
    let offset = match doc.offset_at(position) {
        Some(offset) => offset,
        None => return Ok(None),
    };

    match params.context {
        Some(ctx) => match ctx.trigger_character.as_ref() {
            Some(char) => match char.as_str() {
                "." => use_completion_marker(db, file, position, offset),
                _ => use_completion_marker(db, file, position, offset),
            },
            None => use_completion_marker(db, file, position, offset),
        },
        None => use_completion_marker(db, file, position, offset),
    }
}

pub fn use_completion_marker(db: &impl BaseDatabase, file: File, position: lsp_types::Position, offset: usize) -> anyhow::Result<Option<CompletionResponse>> {
    let mut doc = (**file.document(db)).clone();

    let changes = vec![lsp_types::TextDocumentContentChangeEvent {
        range: Some(lsp_types::Range {
            start: position,
            end: position,
        }),
        range_length: Some(COMPLETION_MARKER.len() as u32),
        text: COMPLETION_MARKER.into(),
    }];

    doc.update(&mut file.parsers(db).parser.write(), &changes)?;

    let file = File::new(db, file.url(db).clone(), file.parsers(db), Arc::new(doc), None);

    use_completion_ctx(db, file, offset)
}

pub fn use_completion_ctx(db: &impl BaseDatabase, file: File, offset: usize) -> anyhow::Result<Option<CompletionResponse>> {
    let ns = match semantic_index(db, file) {
        Some(ns) => ns,
        None => return Ok(None),
    };
    if let Some(symbol) = ns.descendant_at(db, &ns, offset) {
        if let Some(ctx) = symbol.completion(db, offset) {
            return Ok(Some(CompletionResponse::Array(ctx)));
        }
    }
    Ok(Some(CompletionResponse::Array(vec![])))
}
