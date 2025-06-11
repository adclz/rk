#![allow(deprecated)]

use std::sync::Arc;

use ast::generated::{ClassDecl, ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl, ConfigDecl_NamespaceDecl_ProgDecl, FbDecl, FuncDecl, NamespaceDecl, SourceFile};
use auto_lsp::{
    anyhow, core::{
        ast::AstNode, dispatch, dispatch_once, document::Document, document_symbols_builder::DocumentSymbolsBuilder
    }, default::db::{tracked::{get_ast, ParsedAst}, BaseDatabase, File}, lsp_types::{self, CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, SymbolKind}
};

use crate::capabilties::completions::snippets::{class, test, function, function_block, interface, namespace, type_, using};

const COMPLETION_MARKER: &str = "iecCompletionMarker";

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

    completion_context(db, file, offset, &mut results)?;
    Ok(Some(CompletionResponse::Array(results)))
}

fn completion_context(db: &impl BaseDatabase, file: File, offset: usize, results: &mut Vec<CompletionItem>) -> anyhow::Result<()> {
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
    let mut nodes = (file.parsers(db).ast_parser)(db, &clone)?;
    nodes.sort_unstable();
    let ast = ParsedAst { nodes: Arc::new(nodes) };
    let node = ast.descendant_at(offset).unwrap();

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
        } else if lower.is::<ast::generated::UsingDirective>() {
            results.push(test());
            break;
        }
        
        else if lower.is::<ast::generated::FuncDecl>() || 
                  lower.is::<ast::generated::FbDecl>() ||
                  lower.is::<ast::generated::ClassDecl>() || 
                  lower.is::<ast::generated::DataTypeDecl>() || 
                  lower.is::<ast::generated::InterfaceDecl>() {
            break;
        }
        node = parent.get_parent(&ast);
    }
    Ok(())
}
