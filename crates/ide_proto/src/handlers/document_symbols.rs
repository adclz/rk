use auto_lsp::core::document_symbols_builder::DocumentSymbolsBuilder;
use db::WorkspaceDataBase;
use hir::hir_def::{interned::namespace::NamespaceAccess, namespace::NamespaceDecl, pous::pou::Pou};

use crate::handlers::DocumentSymbolsHandler;

impl<'db> DocumentSymbolsHandler<'db> for NamespaceDecl<'db> {
    fn document_symbols(
            &'db self,
            db: &'db dyn WorkspaceDataBase,
            builder: &mut DocumentSymbolsBuilder,
        ) {
        
    }
}

impl<'db> DocumentSymbolsHandler<'db> for Pou<'db> {
    fn document_symbols(
            &'db self,
            db: &'db dyn WorkspaceDataBase,
            builder: &mut DocumentSymbolsBuilder,
        ) {
        
    }
}