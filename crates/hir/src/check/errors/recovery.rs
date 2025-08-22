use std::collections::HashSet;

use auto_lsp::default::db::{BaseDatabase, file::File};

use crate::def::import_map::{Query, global_symbol_indexes};

pub fn pou_recovery(db: &dyn BaseDatabase, file: File, query: &str) -> HashSet<String> {
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
