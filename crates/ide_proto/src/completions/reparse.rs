use std::sync::Arc;

use ast::generated::{FbDecl, FuncDecl, NamespaceDecl};
use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, file::File, tracked::get_ast},
    lsp_types::{CompletionResponse, Position, Range, TextDocumentContentChangeEvent},
};
use hir::hir_def::semantic_index::semantic_index;

use crate::{AsProtocol, completions};

static COMPLETION_MARKER: &str = "$0";

/// Generic helper function to get completions for any AST node that implements AstNode
fn try_get_completions_for_node<T: AstNode>(
    db: &impl BaseDatabase,
    file: File,
    node: &T,
    offset: usize,
) -> anyhow::Result<Option<Vec<auto_lsp::lsp_types::CompletionItem>>> {
    if let Some(hir_node) = semantic_index(db, file).descendant_at(db, node.get_range().start_byte)
    {
        if let Some(completions) = hir_node.as_proto().completion(db, offset) {
            return Ok(Some(completions));
        }
    }
    Ok(None)
}

pub fn use_completion_marker(
    db: &impl BaseDatabase,
    file: File,
    position: Position,
    offset: usize,
) -> anyhow::Result<Option<CompletionResponse>> {
    let mut doc = (**file.document(db)).clone();

    let changes = vec![TextDocumentContentChangeEvent {
        range: Some(Range {
            start: position,
            end: position,
        }),
        range_length: Some(COMPLETION_MARKER.len() as u32),
        text: COMPLETION_MARKER.into(),
    }];

    doc.update(&mut file.parsers(db).parser.write(), &changes)?;

    let new_file = File::new(
        db,
        file.url(db).clone(),
        file.parsers(db),
        Arc::new(doc),
        None,
    );

    use_completion_ctx(db, file, new_file, offset)
}

pub fn use_completion_ctx(
    db: &impl BaseDatabase,
    old_file: File,
    new_file: File,
    offset: usize,
) -> anyhow::Result<Option<CompletionResponse>> {
    let ast = get_ast(db, new_file);
    if let Some(mut node) = ast.descendant_at(offset) {
        loop {
            // Try different node types and get completions if found
            let completions = if let Some(ns) = node.lower().downcast_ref::<NamespaceDecl>() {
                try_get_completions_for_node(db, old_file, ns, offset)?
            } else if let Some(func) = node.lower().downcast_ref::<FuncDecl>() {
                try_get_completions_for_node(db, old_file, func, offset)?
            } else if let Some(fb) = node.lower().downcast_ref::<FbDecl>() {
                try_get_completions_for_node(db, old_file, fb, offset)?
            } else {
                None
            };

            if let Some(completions) = completions {
                return Ok(Some(CompletionResponse::Array(completions)));
            }

            // Move to parent for next iteration
            if let Some(parent) = node.get_parent(ast) {
                node = parent;
            } else {
                break;
            }
        }
    };
    Ok(Some(CompletionResponse::Array(vec![
        completions::static_snippets::namespace(),
        completions::static_snippets::function(),
        completions::static_snippets::function_block(),
        completions::static_snippets::type_(),
        completions::static_snippets::class(),
        completions::static_snippets::interface(),
    ])))
}
