use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder, default::db::BaseDatabase,
    lsp_types::SymbolKind,
};
use hir::{hir_def::pous::interface::MethodPrototype, hir_ty::inheritance_solver::MethodRef, HirNodeInfo};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for MethodRef<'db> {
    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        self.variables(db)
            .iter()
            .for_each(|var| var.document_symbols(db, &mut nested_builder));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name: self.name(db).text(db).to_string(),
            detail: Some("method".to_string()),
            kind: SymbolKind::METHOD,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).unwrap().lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }
}
