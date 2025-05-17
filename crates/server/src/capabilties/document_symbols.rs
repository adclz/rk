#![allow(deprecated)]

use ast::generated::{ClassDecl, ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl, ConfigDecl_NamespaceDecl_ProgDecl, DataTypeDecl, FbDecl, FuncDecl, NamespaceDecl, SourceFile};
use auto_lsp::{
    anyhow,
    core::{
        ast::AstNode,
        dispatch,
        document::Document,
        document_symbols_builder::DocumentSymbolsBuilder,
        salsa::db::{BaseDatabase, File},
    },
    lsp_types::{DocumentSymbol, SymbolKind},
};

pub fn dispatch_document_symbols(
    db: &impl BaseDatabase,
    file: File,
    node: &dyn AstNode,
    builder: &mut DocumentSymbolsBuilder,
) -> anyhow::Result<()> {
    let doc = file.document(db).read();
    dispatch!(
        node,
        [
            SourceFile => symbols(&*doc, builder)
        ]
    );
    Ok(())
}

trait Symbols {
    fn symbols(&self, doc: &Document, builder: &mut DocumentSymbolsBuilder) -> anyhow::Result<()>;
}

impl Symbols for SourceFile {
    fn symbols(&self, doc: &Document, builder: &mut DocumentSymbolsBuilder) -> anyhow::Result<()> {
        type Children = ConfigDecl_NamespaceDecl_ProgDecl;

        self.children
            .iter()
            .try_for_each(|child| {
                match child.as_ref() {
                    Children::NamespaceDecl(n) => n.symbols(doc, builder),
                    _ => Ok(()),
                }
            })
    }
}

impl Symbols for NamespaceDecl {
    fn symbols(&self, doc: &Document, builder: &mut DocumentSymbolsBuilder) -> anyhow::Result<()> {
        type Children = ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

        let mut nested_builder = DocumentSymbolsBuilder::default();

        if let Some(elements) = self.elements.as_ref() {
            elements.children.iter().try_for_each(|child| {
                match child.as_ref() {
                    Children::NamespaceDecl(n) => n.symbols(doc, &mut nested_builder),
                    Children::FbDecl(d) => d.symbols(doc, &mut nested_builder),
                    Children::FuncDecl(d) => d.symbols(doc, &mut nested_builder),
                    _ => Ok(()),
                }
            })
        } else {
            Ok(())
        }?;

        builder.push_symbol(DocumentSymbol {
            name: self.name.get_text(&doc.texter.text.as_bytes())?.to_string(),
            detail: None,
            kind: SymbolKind::NAMESPACE,
            selection_range: self.name.get_lsp_range(),
            deprecated: None,
            tags: None,
            children: Some(nested_builder.finalize()),
            range: self.get_lsp_range(),
        });
        Ok(())
    }
}


impl Symbols for FuncDecl {
    fn symbols(&self, doc: &Document, builder: &mut DocumentSymbolsBuilder) -> anyhow::Result<()> {
        builder.push_symbol(DocumentSymbol {
            name: self.name.get_text(&doc.texter.text.as_bytes())?.to_string(),
            detail: None,
            kind: SymbolKind::FUNCTION,
            selection_range: self.name.get_lsp_range(),
            deprecated: None,
            tags: None,
            children: None,
            range: self.get_lsp_range(),
        });
        Ok(())
    }
}

impl Symbols for FbDecl {
    fn symbols(&self, doc: &Document, builder: &mut DocumentSymbolsBuilder) -> anyhow::Result<()> {
        builder.push_symbol(DocumentSymbol {
            name: self.name.get_text(&doc.texter.text.as_bytes())?.to_string(),
            detail: None,
            kind: SymbolKind::FUNCTION,
            selection_range: self.name.get_lsp_range(),
            deprecated: None,
            tags: None,
            children: None,
            range: self.get_lsp_range(),
        });
        Ok(())
    }
}