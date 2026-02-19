use std::ops::ControlFlow;

use db::WorkspaceDataBase;
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
        file::{file_symbol_index, std_lib_symbol_index},
        query::{Query, SymbolKind},
        variables::variable_symbol_index,
    },
};

pub struct ScopeSearchCtx<
    'db,
    F: Fn(&Pou<'db>, &'db dyn WorkspaceDataBase) -> bool = fn(
        &Pou<'db>,
        &'db dyn WorkspaceDataBase,
    ) -> bool,
> {
    scope: ScopeId<'db>,
    query: Option<Query>,
    include_variables: bool,
    include_pous: bool,
    filter: F,
}

impl<'db, F: Fn(&Pou<'db>, &'db dyn WorkspaceDataBase) -> bool> ScopeSearchCtx<'db, F> {
    /// Create a new search context for a scope
    pub fn new(scope: ScopeId<'db>, filter: F) -> Self {
        Self {
            scope,
            query: None,
            include_variables: true,
            include_pous: true,
            filter,
        }
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

    /// Only search for variables (skip POUs)
    pub fn only_variables(mut self) -> Self {
        self.include_variables = true;
        self.include_pous = false;
        self
    }

    /// Only search for POUs (skip variables)
    pub fn only_pous(mut self) -> Self {
        self.include_variables = false;
        self.include_pous = true;
        self
    }

    /// Execute the search and return results
    pub fn search(self, db: &'db dyn WorkspaceDataBase) -> ScopeSearchResult<'db> {
        let query = self.query.unwrap_or_else(|| Query::new(String::new()));

        let local = discover_in_scope(db, self.scope);
        let mut search_result = ScopeSearchResult::default();

        // Phase 1: Collect variables from current scope
        let scope_variables = if self.include_variables {
            collect_scope_variables(db, self.scope, &mut search_result, &query)
        } else {
            FxHashSet::default()
        };

        // Phase 2: Search for POUs, excluding those shadowed by variables
        if self.include_pous {
            search_pous(
                db,
                &query,
                self.scope,
                &local,
                &scope_variables,
                &self.filter,
                &mut search_result,
            );
        }

        search_result
    }
}

/// A symbol found in scope, either a variable, a locally accessible POU, or one that needs imports
#[derive(Clone)]
pub enum ScopedSymbol<'db> {
    Variable(VariableDecl<'db>),
    LocalPou(Pou<'db>),
    ImportedPou(NamespacePath, Pou<'db>),
}

#[derive(Default)]
pub struct ScopeSearchResult<'db> {
    /// All symbols found in scope (variables, POUs, etc.)
    pub symbols: Vec<ScopedSymbol<'db>>,
}

impl<'db> ScopeSearchResult<'db> {
    /// Get all variables from the result
    pub fn variables(&self) -> impl Iterator<Item = &VariableDecl<'db>> {
        self.symbols.iter().filter_map(|s| match s {
            ScopedSymbol::Variable(v) => Some(v),
            _ => None,
        })
    }

    /// Get all locally accessible POUs (no imports needed)
    pub fn local_pous(&self) -> impl Iterator<Item = &Pou<'db>> {
        self.symbols.iter().filter_map(|s| match s {
            ScopedSymbol::LocalPou(p) => Some(p),
            _ => None,
        })
    }

    /// Get all POUs that need imports (with their namespace)
    pub fn imported_pous(&self) -> impl Iterator<Item = (NamespacePath, Pou<'db>)> + '_ {
        self.symbols.iter().filter_map(|s| match s {
            ScopedSymbol::ImportedPou(ns, p) => Some((*ns, *p)),
            _ => None,
        })
    }
}

/// Collect variables from the current scope that match the query
fn collect_scope_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    search_result: &mut ScopeSearchResult<'db>,
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
            search_result.symbols.push(ScopedSymbol::Variable(v));
        }
        ControlFlow::Continue::<()>(())
    });

    all_variable_names
}

/// Search for POUs that match the query and aren't shadowed by variables
fn search_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    fast_query: &Query,
    scope: ScopeId<'db>,
    local: &LocalSearchResult<'db>,
    scope_variables: &FxHashSet<String>,
    filter_pou: &dyn Fn(&Pou<'db>, &'db dyn WorkspaceDataBase) -> bool,
    search_result: &mut ScopeSearchResult<'db>,
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
        if let SymbolKind::Pou(pou) = symbol.kind {
            // Skip if already defined locally in this scope
            if local.pous.contains_key(&pou.get_name_ident(db)) {
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

            // Handle namespace imports
            if let Some(ns) = symbol.namespace {
                if local.seen_namespaces.contains(&ns) {
                    search_result.symbols.push(ScopedSymbol::LocalPou(pou));
                } else {
                    search_result
                        .symbols
                        .push(ScopedSymbol::ImportedPou(ns, pou));
                }
            } else {
                search_result.symbols.push(ScopedSymbol::LocalPou(pou));
            }
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
