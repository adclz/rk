use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    HasName,
    hir_def::{
        interned::{identifier::Ident, namespace::NamespacePath},
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::name_res::namespace_index,
    query_string::{
        file::file_symbol_index,
        query::{Query, SymbolKind},
        variables::variable_symbol_index,
    },
};

#[derive(Default)]
pub struct ScopeSearchResult<'db> {
    /// POUs that need to be imported via USING directives
    pub need_imports: Vec<(NamespacePath, Pou<'db>)>,
    pub local_variables: Vec<VariableDecl<'db>>,
    pub local_pous: Vec<Pou<'db>>,
}

/// Discover POUs and variables available for a given query in the given scope
///
/// This will return both local POUs and POUs that can be imported via USING directives
pub fn query_scope_items<'db>(
    db: &'db dyn BaseDatabase,
    query: &str,
    scope: ScopeId<'db>,
    filter_pou: impl Fn(&Pou<'db>) -> bool,
) -> ScopeSearchResult<'db> {
    let local = discover_in_scope(db, scope);
    let mut search_result = ScopeSearchResult::default();

    // Collects all symbol indexes to search
    let mut indexes: Vec<_> = db
        .get_files()
        .iter()
        .map(|file| file_symbol_index(db, *file))
        .collect();

    // Add variable index for the current scope
    indexes.push(variable_symbol_index(db, scope));

    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, &indexes, |symbol| {
        match symbol.kind {
            SymbolKind::Pou(pou) => {
                if let Some(ns) = symbol.namespace {
                    // Check if we have already seen this symbol in the local scopes
                    if local.seen_namespaces.contains(&ns) {
                        return ControlFlow::Continue::<()>(());
                    }

                    // Then, check if we have already added this POU
                    if local.pous.contains_key(&pou.get_name_ident(db)) {
                        return ControlFlow::Continue::<()>(());
                    }

                    // Apply the filter
                    if !filter_pou(&pou) {
                        return ControlFlow::Continue::<()>(());
                    }

                    search_result.need_imports.push((ns, pou));
                    return ControlFlow::Continue::<()>(());
                }
                // Apply the filter
                if !filter_pou(&pou) {
                    return ControlFlow::Continue::<()>(());
                }
                search_result.local_pous.push(pou);
            }
            SymbolKind::Variable(v) => {
                search_result.local_variables.push(v);
            }
            _ => {}
        }
        ControlFlow::Continue::<()>(())
    });

    search_result
}

#[derive(Default)]
pub struct LocalSearchResult<'db> {
    pub seen_namespaces: FxHashSet<NamespacePath>,
    pub pous: FxHashMap<Ident, Pou<'db>>,
}

// Iterate through the local scopes and collect local POUs and seen namespaces
pub fn discover_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    scope: ScopeId<'db>,
) -> LocalSearchResult<'db> {
    let mut result = LocalSearchResult::default();

    // Adds pous declared
    scope.def_map(db).local_pous.iter().for_each(|(n, p)| {
        result.pous.insert(*n, *p);
    });

    let it = semantic_index(db, scope.file(db)).scope_iterator(db, scope);
    for scope in it {
        // Find POUs in all shared namespaces
        if let ScopeKind::Namespace(ns) = scope.kind {
            for ns in namespace_index(db, *ns.path(db)).iter() {
                result.seen_namespaces.insert(*ns.path(db));
            }
        }

        // Find POUs in all USING directives
        for using in &scope.usings {
            for ns in namespace_index(db, *using.path(db)).iter() {
                result.seen_namespaces.insert(*ns.path(db));
            }
        }
    }
    result
}
