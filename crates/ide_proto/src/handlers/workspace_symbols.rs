use std::ops::ControlFlow;

use auto_lsp::lsp_types::{Location, OneOf, SymbolKind, WorkspaceSymbol};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::pous::pou::Pou,
    query_string::{
        file::{file_symbol_index, library_symbol_index},
        query::{self, NamedSymbol, Query, SymbolIndex},
    },
};

const MAX_RESULTS: usize = 128;

/// How well a name answers the query, best first. Subsequence matching is
/// what a symbol search wants — `MotorController` and `StepperMotor` both
/// answer `Motor` — but it also lets `test_expt_matches_operator` in, so the
/// order has to say which is which. Ranking, not filtering: switching to
/// [`SearchMode::Similar`] drops both wanted names and answers a three
/// letter query with nothing.
fn rank(name: &str, query: &str) -> u8 {
    let name = name.to_lowercase();
    let query = query.to_lowercase();
    if name == query {
        0
    } else if name.starts_with(&query) {
        1
    } else if name.contains(&query) {
        2
    } else {
        3
    }
}

/// Scanned before ranking, so a good match found late still outranks a poor
/// one found early.
const MAX_SCANNED: usize = MAX_RESULTS * 8;

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
    indices.push(library_symbol_index(db));

    let mut results: Vec<WorkspaceSymbol> = Vec::new();

    search.search(db, &indices, |symbol: &NamedSymbol<'_>| {
        if results.len() >= MAX_SCANNED {
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
            query::SymbolKind::Method(_) => SymbolKind::METHOD,
        };

        let container_name = symbol.namespace.as_ref().map(|ns| ns.to_string(db));

        results.push(WorkspaceSymbol {
            name: symbol.name.clone(),
            kind: lsp_kind,
            tags: None,
            container_name,
            location: OneOf::Left(Location::new(
                file.url(db).clone(),
                hir::denormalize(db, file, &span).unwrap_or_default(),
            )),
            data: None,
        });

        ControlFlow::Continue(())
    });

    results.sort_by(|a, b| {
        rank(&a.name, query_str)
            .cmp(&rank(&b.name, query_str))
            .then(a.name.len().cmp(&b.name.len()))
            .then(a.name.cmp(&b.name))
    });
    results.truncate(MAX_RESULTS);
    results
}
