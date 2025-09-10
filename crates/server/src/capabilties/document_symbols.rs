#![allow(deprecated)]

use std::ops::ControlFlow;

use auto_lsp::{
    anyhow,
    core::{document::Document, document_symbols_builder::DocumentSymbolsBuilder},
    default::db::BaseDatabase,
    lsp_types::{
        DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, SymbolKind, SymbolTag,
    },
};
use hir::{
    hir_def::{
        namespace::NamespaceDecl,
        pous::{
            class::MethodDecl, interface::MethodPrototype, pou::{Pou, PouDecl}, variable::VariableDecl
        },
        semantic_index::{semantic_index, HirNode},
    },
    to_proto::ToProto,
    walk::WalkHir,
};

pub fn document_symbols(
    db: &impl BaseDatabase,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let mut builder = DocumentSymbolsBuilder::default();

    let sema = semantic_index(db, file);

    sema.global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(db, &mut builder));
    sema.namespaces
        .iter()
        .for_each(|ns| ns.document_symbols(db, &mut builder));

    Ok(Some(DocumentSymbolResponse::Nested(builder.finalize())))
}
