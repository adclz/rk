use std::ops::ControlFlow;

use db::WorkspaceDataBase;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    HasName,
    hir_def::{
        interned::{identifier::Ident, namespace::NamespacePath},
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::index_graphs::namespace_index,
    query_string::{
        file::{file_symbol_index, std_lib_symbol_index},
        query::{Query, SymbolKind},
        variables::variable_symbol_index,
    },
};

pub struct SymbolSearch<
    'db,
    F: Fn(&Pou<'db>, &'db dyn WorkspaceDataBase) -> bool = fn(
        &Pou<'db>,
        &'db dyn WorkspaceDataBase,
    ) -> bool,
> {
    scope: Option<ScopeId<'db>>,
    query: Option<Query>,
    include_variables: bool,
    include_pous: bool,
    include_namespaces: bool,
    filter: F,
}

impl<'db, F: Fn(&Pou<'db>, &'db dyn WorkspaceDataBase) -> bool> SymbolSearch<'db, F> {
    /// Create a new symbol search with a POU filter
    pub fn new(filter: F) -> Self {
        Self {
            scope: None,
            query: None,
            include_variables: true,
            include_pous: true,
            include_namespaces: false,
            filter,
        }
    }

    /// Set the scope for variable search and import categorization
    pub fn with_scope(mut self, scope: ScopeId<'db>) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Set the query to search for
    pub fn with_query(mut self, query: Query) -> Self {
        self.query = Some(query);
        self
    }

    /// Include variables in the search (default: true)
    pub fn with_variables(mut self, include: bool) -> Self {
        self.include_variables = include;
        self
    }

    /// Include POUs in the search (default: true)
    pub fn with_pous(mut self, include: bool) -> Self {
        self.include_pous = include;
        self
    }

    /// Include namespaces in the search (default: false)
    pub fn with_namespaces(mut self, include: bool) -> Self {
        self.include_namespaces = include;
        self
    }

    /// Only search for variables (skip POUs and namespaces)
    pub fn only_variables(mut self) -> Self {
        self.include_variables = true;
        self.include_pous = false;
        self.include_namespaces = false;
        self
    }

    /// Only search for POUs (skip variables and namespaces)
    pub fn only_pous(mut self) -> Self {
        self.include_variables = false;
        self.include_pous = true;
        self.include_namespaces = false;
        self
    }

    /// Only search for namespaces (skip variables and POUs)
    pub fn only_namespaces(mut self) -> Self {
        self.include_variables = false;
        self.include_pous = false;
        self.include_namespaces = true;
        self
    }

    /// Execute the search and return results
    pub fn search(self, db: &'db dyn WorkspaceDataBase) -> SearchResult<'db> {
        let query = self.query.unwrap_or_else(|| Query::new(String::new()));
        let mut search_result = SearchResult::default();

        // Phase 1: Collect variables from current scope (only if scope is set)
        let scope_variables = if self.include_variables {
            if let Some(scope) = self.scope {
                collect_scope_variables(db, scope, &mut search_result, &query)
            } else {
                FxHashSet::default()
            }
        } else {
            FxHashSet::default()
        };

        // Phase 2: Search for POUs and/or namespaces in file+stdlib indexes
        if self.include_pous || self.include_namespaces {
            let local = self.scope.map(|s| discover_in_scope(db, s));

            search_file_indexes(
                db,
                &query,
                self.scope,
                local.as_ref(),
                &scope_variables,
                &self.filter,
                self.include_pous,
                self.include_namespaces,
                &mut search_result,
            );
        }

        search_result
    }
}

/// A symbol found by the search
#[derive(Clone)]
pub enum SearchSymbol<'db> {
    Variable(VariableDecl<'db>),
    LocalPou(Pou<'db>),
    ImportedPou(NamespacePath, Pou<'db>),
    Namespace(NamespaceDecl<'db>),
}

#[derive(Default)]
pub struct SearchResult<'db> {
    /// All symbols found
    pub symbols: Vec<SearchSymbol<'db>>,
}

impl<'db> SearchResult<'db> {
    /// Get all variables from the result
    pub fn variables(&self) -> impl Iterator<Item = &VariableDecl<'db>> {
        self.symbols.iter().filter_map(|s| match s {
            SearchSymbol::Variable(v) => Some(v),
            _ => None,
        })
    }

    /// Get all locally accessible POUs (no imports needed)
    pub fn local_pous(&self) -> impl Iterator<Item = &Pou<'db>> {
        self.symbols.iter().filter_map(|s| match s {
            SearchSymbol::LocalPou(p) => Some(p),
            _ => None,
        })
    }

    /// Get all POUs that need imports (with their namespace)
    pub fn imported_pous(&self) -> impl Iterator<Item = (NamespacePath, Pou<'db>)> + '_ {
        self.symbols.iter().filter_map(|s| match s {
            SearchSymbol::ImportedPou(ns, p) => Some((*ns, *p)),
            _ => None,
        })
    }

    /// Get all namespace declarations from the result
    pub fn namespaces(&self) -> impl Iterator<Item = &NamespaceDecl<'db>> {
        self.symbols.iter().filter_map(|s| match s {
            SearchSymbol::Namespace(ns) => Some(ns),
            _ => None,
        })
    }
}

/// Collect variables from the current scope that match the query
fn collect_scope_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    search_result: &mut SearchResult<'db>,
    fast_query: &Query,
) -> FxHashSet<String> {
    // Collect ALL variable names first for deduplication
    let mut all_variable_names = FxHashSet::default();
    scope
        .def_map(db)
        .global_variables
        .iter()
        .for_each(|(_, v)| {
            all_variable_names.insert(v.name(db).text(db).to_string());
        });

    // Now search for variables that match the query
    let index = variable_symbol_index(db, scope);
    let indexes = vec![index];

    fast_query.search(db, &indexes, |symbol| {
        if let SymbolKind::Variable(v) = symbol.kind {
            search_result.symbols.push(SearchSymbol::Variable(v));
        }
        ControlFlow::Continue::<()>(())
    });

    all_variable_names
}

/// Search file+stdlib indexes for POUs and/or namespaces
fn search_file_indexes<'db>(
    db: &'db dyn WorkspaceDataBase,
    fast_query: &Query,
    scope: Option<ScopeId<'db>>,
    local: Option<&LocalSearchResult<'db>>,
    scope_variables: &FxHashSet<String>,
    filter_pou: &dyn Fn(&Pou<'db>, &'db dyn WorkspaceDataBase) -> bool,
    include_pous: bool,
    include_namespaces: bool,
    search_result: &mut SearchResult<'db>,
) {
    // Collect per-file symbol indexes for workspace files + single stdlib index
    let mut indexes: Vec<_> = db
        .get_files()
        .iter()
        .map(|file| file_symbol_index(db, *file))
        .collect();
    indexes.push(std_lib_symbol_index(db));

    // Track POUs we've already added to avoid duplicates from the same namespace
    let mut seen_pous: FxHashSet<(Option<NamespacePath>, String)> = FxHashSet::default();

    fast_query.search(db, &indexes, |symbol| {
        match symbol.kind {
            SymbolKind::Pou(pou) if include_pous => {
                // Skip if already defined locally in this scope
                if let Some(local) = local
                    && local.pous.contains_key(&pou.get_name_ident(db)) {
                        return ControlFlow::Continue::<()>(());
                    }

                // Skip if a variable with the same name exists (variables take priority)
                if scope_variables.contains(&symbol.name) {
                    return ControlFlow::Continue::<()>(());
                }

                // Apply the user-provided filter
                if !filter_pou(&pou, db) {
                    return ControlFlow::Continue::<()>(());
                }

                // Skip if we've already added a POU with this name from this namespace
                let pou_key = (symbol.namespace, symbol.name.clone());
                if !seen_pous.insert(pou_key) {
                    return ControlFlow::Continue::<()>(());
                }

                // Handle namespace imports (only when scope context is available)
                if let Some(local) = local {
                    if let Some(ns) = symbol.namespace {
                        if local.seen_namespaces.contains(&ns) {
                            search_result.symbols.push(SearchSymbol::LocalPou(pou));
                        } else {
                            search_result
                                .symbols
                                .push(SearchSymbol::ImportedPou(ns, pou));
                        }
                    } else {
                        search_result.symbols.push(SearchSymbol::LocalPou(pou));
                    }
                } else {
                    // No scope context: all POUs are treated as local
                    search_result.symbols.push(SearchSymbol::LocalPou(pou));
                }
            }
            SymbolKind::Namespace(ns) if include_namespaces => {
                search_result.symbols.push(SearchSymbol::Namespace(ns));
            }
            _ => {}
        }
        ControlFlow::Continue::<()>(())
    });
}

#[derive(Default)]
pub struct LocalSearchResult<'db> {
    pub seen_namespaces: FxHashSet<NamespacePath>,
    pub pous: FxHashMap<Ident, Pou<'db>>,
}

// Iterate through the local scopes and collect local POUs and seen namespaces
pub fn discover_in_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
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
