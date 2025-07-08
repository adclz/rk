#![allow(deprecated)]

use std::sync::Arc;

use auto_lsp::{
    anyhow, core::ast::AstNode, default::db::{tracked::ParsedAst, BaseDatabase, file::File}, lsp_types::{self, CompletionItem, CompletionParams, CompletionResponse}
};

use crate::capabilties::completions::snippets::{class, function, function_block, interface, namespace, test, type_, using, var, var_input, var_output, var_temp};

const COMPLETION_MARKER: &str = "iecCompletionMarker";

pub fn closest(nodes: &[Arc<dyn AstNode>], offset: usize) -> Option<&Arc<dyn AstNode>> {
    let mut result = None;
    for node in nodes {
        let range = node.get_range();
        
        if range.start_byte >= offset {
            result = Some(node);
            break;
        }
    }
    result
}

pub fn completions(
    db: &impl BaseDatabase,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let doc = file.document(db);

    let mut results = vec![];

    Ok(Some(CompletionResponse::Array(results)))
}