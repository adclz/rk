use auto_lsp::default::db::{BaseDatabase, file::File};
use auto_lsp::lsp_types::{CompletionItem, CompletionItemKind};
use fst::{Automaton, Streamer, raw::IndexedValue};
use rayon::prelude::*;
use rustc_hash::FxHashSet;

use std::ops::ControlFlow;
use std::{cmp::Ordering, hash::Hash};

use crate::hir_ty::name_res::pou_names_res;
use crate::HirNodeInfo;
use crate::hir_def::pous::pou::PouDecl;
use crate::hir_def::scope::FileScopeId;
use crate::hir_def::semantic_index::semantic_index;
use crate::hir_ty::ty::Ty;

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

    pub fn search<'sym, T>(
        &self,
        db: &'sym dyn BaseDatabase,
        indices: Vec<SymbolIndex<'sym>>,
        cb: impl FnMut(&'sym NamedSymbol) -> ControlFlow<T>,
    ) -> Option<T> {
        let mut op = fst::map::OpBuilder::new();
        match self.mode {
            SearchMode::Exact => {
                let automaton = fst::automaton::Str::new(&self.lowercased);

                for index in indices.iter() {
                    op = op.add(index.map(db).search(&automaton));
                }
                self.search_maps(db, indices, op.union(), cb)
            }
            SearchMode::Fuzzy => {
                let automaton = fst::automaton::Subsequence::new(&self.lowercased);

                for index in indices.iter() {
                    op = op.add(index.map(db).search(&automaton));
                }
                self.search_maps(db, indices, op.union(), cb)
            }
            SearchMode::Prefix => {
                let automaton = fst::automaton::Str::new(&self.lowercased).starts_with();

                for index in indices.iter() {
                    op = op.add(index.map(db).search(&automaton));
                }
                self.search_maps(db, indices, op.union(), cb)
            }
        }
    }

    fn search_maps<'sym, T>(
        &self,
        db: &'sym dyn BaseDatabase,
        indices: Vec<SymbolIndex<'sym>>,
        mut stream: fst::map::Union<'_>,
        mut cb: impl FnMut(&'sym NamedSymbol) -> ControlFlow<T>,
    ) -> Option<T> {
        while let Some((_, indexed_values)) = stream.next() {
            for &IndexedValue { index, value } in indexed_values {
                let symbol_index = &indices[index];
                let (start, end) = SymbolIndex::map_value_to_range(value);

                for symbol in &symbol_index.symbols(db)[start..end] {
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

#[salsa::tracked]
pub struct SymbolIndex<'db> {
    #[tracked]
    #[returns(ref)]
    symbols: Box<[NamedSymbol<'db>]>,
    
    #[tracked]
    #[no_eq]
    #[returns(ref)]
    map: fst::Map<Vec<u8>>,
}

impl<'db> SymbolIndex<'db> {
    pub fn create(
        db: &'db dyn BaseDatabase,
        mut symbols: Box<[NamedSymbol<'db>]>,
    ) -> SymbolIndex<'db> {
        fn cmp(lhs: &NamedSymbol, rhs: &NamedSymbol) -> Ordering {
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
        SymbolIndex::new(db, symbols, map)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct NamedSymbol<'db> {
    pub name: String,
    pub kind: SymbolKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SymbolKind<'db> {
    Pou(PouDecl<'db>),
    StructField(Ty<'db>),
    Variable(Ty<'db>),
}

impl<'db> HirNodeInfo<'db> for NamedSymbol<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> crate::AstId {
        match self.kind {
            SymbolKind::Pou(p) => p.get_id(db),
            SymbolKind::StructField(ty) => ty.get_id(db),
            SymbolKind::Variable(ty) => ty.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self.kind {
            SymbolKind::Pou(p) => p.get_scope_id(db),
            SymbolKind::StructField(ty) => ty.get_scope_id(db),
            SymbolKind::Variable(ty) => ty.get_scope_id(db),
        }
    }
}

#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_symbol_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SymbolIndex<'db> {
    let mut pous = vec![];
    let sema = semantic_index(db, file);

    sema.namespaces.iter().for_each(|ns| {
        ns.pous(db).iter().for_each(|pou| {
            pous.push(NamedSymbol {
                name: pou.name(db).text(db).to_string(),
                kind: SymbolKind::Pou(*pou),
            });
        });
    });

    SymbolIndex::create(db, pous.into_boxed_slice())
}

pub fn global_symbol_indexes(db: &dyn BaseDatabase, file_to_omit: File) -> Vec<SymbolIndex> {
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
    scope_id: FileScopeId<'_>,
    query: &str,
) -> Vec<CompletionItem> {
    let indexes = global_symbol_indexes(db, file);
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    let mut results = vec![];

    fast_query.search(db, indexes, |symbol| {
        results.push(CompletionItem {
            label: symbol.name.clone(),
            kind: Some(CompletionItemKind::MODULE),
            ..Default::default()
        });

        std::ops::ControlFlow::Continue::<()>(())
    });

    results
}
