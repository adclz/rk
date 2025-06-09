#![allow(deprecated)]

use std::sync::Arc;

use ast::generated::{ClassDecl, ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl, ConfigDecl_NamespaceDecl_ProgDecl, FbDecl, FuncDecl, NamespaceDecl, SourceFile};
use auto_lsp::{
    anyhow, core::{
        ast::AstNode, dispatch, dispatch_once, document::Document, document_symbols_builder::DocumentSymbolsBuilder
    }, default::db::{tracked::{get_ast, ParsedAst}, BaseDatabase, File}, lsp_types::{self, CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, SymbolKind}
};

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


pub fn closest(nodes: &[Arc<dyn AstNode>], offset: usize) -> Option<&Arc<dyn AstNode>> {
        let mut result = None;
        for node in nodes.iter() {
            let range = node.get_range();
            result = Some(node);

            if range.start_byte >= offset {
                break;
            } 
        }
        result
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
    let node = closest(&ast, offset).unwrap();

    eprintln!("{:?} <=> {}", node.get_lsp_range(), offset);

    let mut node = node.get_parent(&ast);

    if node.is_none() {
        results.push(CompletionItem {
                label: "NAMESPACE".into(),
                kind: Some(CompletionItemKind::MODULE),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("NAMESPACE ${1:ns} END_NAMESPACE".into()),
                ..Default::default()
            });
        return Ok(());
    }

    let mut iter_ctr = 0;
    while let Some(parent) = node {
        let lower = parent.lower();
        if let Some(parent) = lower.downcast_ref::<ast::generated::SourceFile>() {
            results.push(CompletionItem {
                label: "NAMESPACE".into(),
                kind: Some(CompletionItemKind::MODULE),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("NAMESPACE ${1:ns} END_NAMESPACE".into()),
                ..Default::default()
            });
            break;
        } else if let Some(parent) = lower.downcast_ref::<ast::generated::NamespaceDecl>() {
            results.push(CompletionItem {
                label: "USING".into(),
                kind: Some(CompletionItemKind::FOLDER),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("USING ${1:ns};".into()),
                ..Default::default()
            });
            results.push(CompletionItem {
                label: "NAMESPACE".into(),
                kind: Some(CompletionItemKind::MODULE),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("NAMESPACE ${1:ns} END_NAMESPACE".into()),
                ..Default::default()
            });
            results.push(CompletionItem {
                label: "FUNCTION".into(),
                kind: Some(CompletionItemKind::FUNCTION),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("FUNCTION ${1:fn} : ${2:BOOL} \n;\nEND_FUNCTION".into()),
                ..Default::default()
            });
            results.push(CompletionItem {
                label: "FUNCTION_BLOCK".into(),
                kind: Some(CompletionItemKind::FUNCTION),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("FUNCTION_BLOCK ${1:fn}  \n;\nEND_FUNCTION_BLOCK".into()),
                ..Default::default()
            });
            results.push(CompletionItem {
                label: "TYPE".into(),
                kind: Some(CompletionItemKind::TYPE_PARAMETER),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("TYPE ${1:type} := ${2:BOOL} \n;\nEND_TYPE".into()),
                ..Default::default()
            });
            results.push(CompletionItem {
                label: "CLASS".into(),
                kind: Some(CompletionItemKind::CLASS),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("CLASS ${1:class} USING ${2:ns} \n;\nEND_CLASS".into()),
                ..Default::default()
            });
            results.push(CompletionItem {
                label: "INTERFACE".into(),
                kind: Some(CompletionItemKind::INTERFACE),
                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                insert_text: Some("INTERFACE ${1:interface} \n;\nEND_INTERFACE".into()),
                ..Default::default()
            });
            break;
        } else if lower.is::<FuncDecl>() || 
                  lower.is::<FbDecl>() ||
                  lower.is::<ClassDecl>() || 
                  lower.is::<ast::generated::DataTypeDecl>() || 
                  lower.is::<ast::generated::InterfaceDecl>() ||
                  lower.is::<ast::generated::ConfigDecl>() {
            eprintln!("BREEEEEAK");
            break;
        }
        node = parent.get_parent(&ast);
        iter_ctr += 1;
        if iter_ctr > 15 {
            break;
        }
    }
    Ok(())
}
