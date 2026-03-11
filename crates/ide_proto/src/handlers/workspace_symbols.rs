use std::ops::ControlFlow;

use auto_lsp::lsp_types::{Location, OneOf, SymbolKind, WorkspaceSymbol};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::pous::pou::Pou,
    query_string::{
        file::{file_symbol_index, std_lib_symbol_index},
        query::{self, NamedSymbol, Query, SymbolIndex},
    },
};

const MAX_RESULTS: usize = 128;

pub fn workspace_symbols(db: &dyn WorkspaceDataBase, query_str: &str) -> Vec<WorkspaceSymbol> {
    if query_str.is_empty() {
        return vec![];
    }

    let mut search = Query::new(query_str.to_string());
    search.fuzzy();

    let mut indices: Vec<SymbolIndex<'_>> = Vec::new();
    for file in db.get_files().iter() {
        indices.push(file_symbol_index(db, *file));
    }
    indices.push(std_lib_symbol_index(db));

    let mut results: Vec<WorkspaceSymbol> = Vec::new();

    search.search(db, &indices, |symbol: &NamedSymbol<'_>| {
        if results.len() >= MAX_RESULTS {
            return ControlFlow::Break(());
        }

        let scope_id = symbol.get_scope_id(db);
        let file = scope_id.file(db);
        let span = symbol.get_span(db);

        let lsp_kind = match &symbol.kind {
            query::SymbolKind::Namespace(_) => SymbolKind::NAMESPACE,
            query::SymbolKind::Pou(pou) => match pou {
                Pou::Function(_) => SymbolKind::FUNCTION,
                Pou::FunctionBlock(_) => SymbolKind::FUNCTION,
                Pou::Class(_) => SymbolKind::CLASS,
                Pou::Interface(_) => SymbolKind::INTERFACE,
                Pou::DataType(_) => SymbolKind::STRUCT,
            },
            query::SymbolKind::StructField(_) => SymbolKind::FIELD,
            query::SymbolKind::Variable(_) => SymbolKind::VARIABLE,
        };

        let container_name = symbol.namespace.as_ref().map(|ns| ns.to_string(db));

        results.push(WorkspaceSymbol {
            name: symbol.name.clone(),
            kind: lsp_kind,
            tags: None,
            container_name,
            location: OneOf::Left(Location::new(file.url(db).clone(), span.into())),
            data: None,
        });

        ControlFlow::Continue(())
    });

    results
}
