#![allow(deprecated)]

use std::sync::Arc;

use ast::generated::{ClassDecl, ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl, ConfigDecl_NamespaceDecl_ProgDecl, FbDecl, FuncDecl, NamespaceDecl, SourceFile};
use auto_lsp::{
    anyhow, core::{
        ast::AstNode, dispatch, dispatch_once, document::Document, document_symbols_builder::DocumentSymbolsBuilder
    }, default::db::{tracked::{get_ast, ParsedAst}, BaseDatabase, File}, lsp_types::{self, CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, SymbolKind}
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
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let doc = file.document(db);

    let offset = doc.offset_at(params.text_document_position.position).unwrap();

    let mut results = vec![];

    completion_context(db, file, offset, &params, &mut results)?;
    Ok(Some(CompletionResponse::Array(results)))
}

fn completion_context(db: &impl BaseDatabase, file: File, offset: usize, params: &CompletionParams, results: &mut Vec<CompletionItem>) -> anyhow::Result<()> {
    let mut clone = (*file.document(db)).clone();
    let start= file.document(db).position_at(offset).unwrap();
    let end = lsp_types::Position::new(start.line, start.character + COMPLETION_MARKER.len() as u32);
    clone.update(&mut file.parsers(db).parser.write(), &[lsp_types::TextDocumentContentChangeEvent {
        range: Some(lsp_types::Range {
            start,
            end,
        }),
        range_length: None,
        text: COMPLETION_MARKER.into(),
    }])?;
    let ast = ParsedAst::new((file.parsers(db).ast_parser)(db, &clone)?);
    let node = match closest(&ast, offset) {
        Some(node) => node,
        None => {
            if ast.get_root().is_none() {
                results.push(namespace());
            }
            return Ok(())
        },
    };

    if let Some(ctx) = &params.context {
        if ctx.trigger_character == Some(".".into()) {
            return Ok(());
        } else {
            no_ctx_completions(&node, &ast, results)?;
        }
    } else {
        no_ctx_completions(&node, &ast, results)?;
    }
    Ok(())
}

fn no_ctx_completions(node: &Arc<dyn AstNode>, ast: &ParsedAst, results: &mut Vec<CompletionItem>) -> anyhow::Result<()>  {
    let mut node = node.get_parent(&ast);

    if node.is_none() {
        results.push(namespace());
        return Ok(());
    }

    while let Some(parent) = node {
        let lower = parent.lower();
        if lower.is::<ast::generated::SourceFile>() {
            results.push(namespace());
            break;
        } else if lower.is::<ast::generated::NamespaceDecl>() {
            results.push(using());
            results.push(namespace());
            results.push(function());
            results.push(function_block());
            results.push(type_());
            results.push(class());
            results.push(interface());
            break;
        } else if lower.is::<ast::generated::FuncDecl>() {
            results.push(var_input());
            results.push(var_output());
            results.push(var_temp());
            results.push(var());
            break;
        } else if lower.is::<ast::generated::UsingDirective>() {
            results.push(test());
            break;
        }
        else if lower.is::<ast::generated::FbDecl>() ||
                  lower.is::<ast::generated::ClassDecl>() || 
                  lower.is::<ast::generated::DataTypeDecl>() || 
                  lower.is::<ast::generated::InterfaceDecl>() {
            break;
        }
        node = parent.get_parent(&ast);
    }
    Ok(())
}