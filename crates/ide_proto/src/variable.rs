use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder, default::db::BaseDatabase,
    lsp_types::SymbolKind,
};
use hir::{HirNodeInfo, hir_def::pous::variable::VariableDecl};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for VariableDecl<'db> {
    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name: self.name(db).text(db).to_string(),
            detail: Some("variable".to_string()),
            kind: SymbolKind::VARIABLE,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).unwrap().lsp(),
            children: None,
            tags: None,
        });
    }
}
