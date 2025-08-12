use std::{collections::HashSet, ops::ControlFlow};

use auto_lsp::default::db::{file::File, BaseDatabase};

use crate::hir::import_map::{global_symbol_indexes, Query};

pub fn pou_recovery<'db>(db: &'db dyn BaseDatabase, file: File, query: &str) -> HashSet<String> {
    query_ident(db, file, query)
}

pub fn query_ident(db: &dyn BaseDatabase, file: File, query: &str) -> HashSet<String> {
    let indexes = global_symbol_indexes(db, file);
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    let mut results = HashSet::new();

    fast_query.search(&indexes, |symbol| {
        results.insert(symbol.name.clone());
        std::ops::ControlFlow::Continue::<()>(())
    });
    results
}