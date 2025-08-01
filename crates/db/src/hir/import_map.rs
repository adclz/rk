use auto_lsp::default::db::{file::File, BaseDatabase};
use auto_lsp::lsp_types::{CompletionItem, CompletionItemKind};
use fst::{raw::IndexedValue, Automaton, Streamer};
use rayon::prelude::*;
use rustc_hash::FxHashSet;

use std::{cmp::Ordering, hash::Hash};
use std::{hash::Hasher, ops::ControlFlow};

use crate::hir::scopes::scope::ScopeId;
use crate::hir::scopes::solver::exported_items_in_scope;
use crate::hir::semantic_index::semantic_index;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SearchMode {
    /// Import map entry should strictly match the query string.
    Exact,
    /// Import map entry should contain all letters from the query string,
    /// in the same order, but not necessary adjacent.
    Fuzzy,
    /// Import map entry should match the query string by prefix.
    Prefix,
}

impl SearchMode {
    pub fn check(self, query: &str, case_sensitive: bool, candidate: &str) -> bool {
        match self {
            SearchMode::Exact if case_sensitive => candidate == query,
            SearchMode::Exact => candidate.eq_ignore_ascii_case(query),
            SearchMode::Prefix => {
                query.len() <= candidate.len() && {
                    let prefix = &candidate[..query.len()];
                    if case_sensitive {
                        prefix == query
                    } else {
                        prefix.eq_ignore_ascii_case(query)
                    }
                }
            }
            SearchMode::Fuzzy => {
                let mut name = candidate;
                query.chars().all(|query_char| {
                    let m = if case_sensitive {
                        name.match_indices(query_char).next()
                    } else {
                        name.match_indices([query_char, query_char.to_ascii_uppercase()])
                            .next()
                    };
                    match m {
                        Some((index, _)) => {
                            name = name[index..]
                                .strip_prefix(|_: char| true)
                                .unwrap_or_default();
                            true
                        }
                        None => false,
                    }
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Query {
    query: String,
    lowercased: String,
    mode: SearchMode,
    case_sensitive: bool,
}

impl Query {
    pub fn new(query: String) -> Query {
        let lowercased = query.to_lowercase();
        Query {
            query,
            lowercased,
            mode: SearchMode::Fuzzy,
            case_sensitive: false,
        }
    }

    pub fn fuzzy(&mut self) {
        self.mode = SearchMode::Fuzzy;
    }

    pub fn exact(&mut self) {
        self.mode = SearchMode::Exact;
    }

    pub fn prefix(&mut self) {
        self.mode = SearchMode::Prefix;
    }

    pub fn case_sensitive(&mut self) {
        self.case_sensitive = true;
    }

    fn search<'sym, T>(
        &self,
        indices: &'sym [&SymbolIndex],
        cb: impl FnMut(&'sym FileSymbol) -> ControlFlow<T>,
    ) -> Option<T> {
        let mut op = fst::map::OpBuilder::new();
        match self.mode {
            SearchMode::Exact => {
                let automaton = fst::automaton::Str::new(&self.lowercased);

                for index in indices.iter() {
                    op = op.add(index.map.search(&automaton));
                }
                self.search_maps(indices, op.union(), cb)
            }
            SearchMode::Fuzzy => {
                let automaton = fst::automaton::Subsequence::new(&self.lowercased);

                for index in indices.iter() {
                    op = op.add(index.map.search(&automaton));
                }
                self.search_maps(indices, op.union(), cb)
            }
            SearchMode::Prefix => {
                let automaton = fst::automaton::Str::new(&self.lowercased).starts_with();

                for index in indices.iter() {
                    op = op.add(index.map.search(&automaton));
                }
                self.search_maps(indices, op.union(), cb)
            }
        }
    }

    fn search_maps<'sym, T>(
        &self,
        indices: &'sym [&SymbolIndex],
        mut stream: fst::map::Union<'_>,
        mut cb: impl FnMut(&'sym FileSymbol) -> ControlFlow<T>,
    ) -> Option<T> {
        while let Some((_, indexed_values)) = stream.next() {
            for &IndexedValue { index, value } in indexed_values {
                let symbol_index = &indices[index];
                let (start, end) = SymbolIndex::map_value_to_range(value);

                for symbol in &symbol_index.symbols[start..end] {
                    let symbol_name = symbol.name.as_str();

                    if let Some(b) = cb(symbol).break_value() {
                        if self
                            .mode
                            .check(&self.query, self.case_sensitive, symbol_name)
                        {
                            return Some(b);
                        }
                    }
                }
            }
        }
        None
    }
}

#[derive(Default)]
pub struct SymbolIndex {
    symbols: Box<[FileSymbol]>,
    map: fst::Map<Vec<u8>>,
}

impl PartialEq for SymbolIndex {
    fn eq(&self, other: &SymbolIndex) -> bool {
        self.symbols == other.symbols
    }
}

impl Eq for SymbolIndex {}

impl Hash for SymbolIndex {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.symbols.hash(hasher)
    }
}

impl SymbolIndex {
    fn new(mut symbols: Box<[FileSymbol]>) -> SymbolIndex {
        fn cmp(lhs: &FileSymbol, rhs: &FileSymbol) -> Ordering {
            let lhs_chars = lhs.name.as_str().chars().map(|c| c.to_ascii_lowercase());
            let rhs_chars = rhs.name.as_str().chars().map(|c| c.to_ascii_lowercase());
            lhs_chars.cmp(rhs_chars)
        }

        symbols.par_sort_by(cmp);

        let mut builder = fst::MapBuilder::memory();

        let mut last_batch_start = 0;

        for idx in 0..symbols.len() {
            if let Some(next_symbol) = symbols.get(idx + 1) {
                if cmp(&symbols[last_batch_start], next_symbol) == Ordering::Equal {
                    continue;
                }
            }

            let start = last_batch_start;
            let end = idx + 1;
            last_batch_start = end;

            let key = symbols[start].name.as_str().to_ascii_lowercase();
            let value = SymbolIndex::range_to_map_value(start, end);

            builder.insert(key, value).unwrap();
        }

        let map = builder
            .into_inner()
            .and_then(|mut buf| {
                fst::Map::new({
                    buf.shrink_to_fit();
                    buf
                })
            })
            .unwrap();
        SymbolIndex { symbols, map }
    }

    fn range_to_map_value(start: usize, end: usize) -> u64 {
        debug_assert![start <= (u32::MAX as usize)];
        debug_assert![end <= (u32::MAX as usize)];

        ((start as u64) << 32) | end as u64
    }

    fn map_value_to_range(value: u64) -> (usize, usize) {
        let end = value as u32 as usize;
        let start = (value >> 32) as usize;
        (start, end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileSymbol {
    pub name: String,
    pub is_import: bool,
    // pub pou: Pou,
}

#[salsa::tracked(returns(ref))]
pub fn file_symbol_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SymbolIndex {
    let mut pous = vec![];

    let sema = semantic_index(db, file).unwrap();

    sema.namespace_keys.iter().for_each(|(key, ns)| {
        ns.pous(db).iter().for_each(|pou| {
            let pou = sema.get_pou(*pou);
            pous.push(FileSymbol {
                name: pou.name(db).text(db).to_string(),
                is_import: false, // This can be set based on some condition
                                  // pou: Pou::from(pou),
            });
        });
    });

    SymbolIndex::new(pous.into_boxed_slice())
}

fn global_symbol_indexes<'db>(db: &'db dyn BaseDatabase, file_to_omit: File) -> Vec<&'db SymbolIndex> {
    db.get_files()
        .iter()
        // Filter out the file to omit
        // Local queries have to be used to know which items are visible in the current scope
        .filter_map(|file| match *file == file_to_omit {
            true => Some(file_symbol_index(db, *file)),
            false => None,
        })
        .collect()
}

pub fn query_completions(
    db: &dyn BaseDatabase,
    file: File,
    scope_id: ScopeId,
    query: &str,
) -> Vec<CompletionItem> {
    let sema = semantic_index(db, file).unwrap();
    let scoped_map = exported_items_in_scope(db, file, scope_id);

    let locally_visible_names: FxHashSet<String> = scoped_map
        .pous
        .keys()
        .map(|ident| ident.text(db).to_owned())
        .collect();

    let indexes = global_symbol_indexes(db, file);
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy(); // or fast_query.exact();
    

    let mut results = vec![];

    fast_query.search(&indexes, |symbol| {
        results.push(CompletionItem {
            label: symbol.name.clone(),
            kind: Some(CompletionItemKind::MODULE),
            ..Default::default()
        });

        std::ops::ControlFlow::Continue::<()>(())
    });

    results
}
