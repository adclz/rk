use std::cmp::Ordering;

use auto_lsp::default::db::{BaseDatabase, file::File};
use rayon::slice::ParallelSliceMut;

use crate::{
    hir_def::semantic_index::semantic_index,
    query_string::query::{NamedSymbol, SymbolKind},
};

#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_pous_symbol_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SymbolIndex<'db> {
    let mut pous = vec![];
    let sema = semantic_index(db, file);

    sema.global_pous.iter().for_each(|pou| {
        pous.push(NamedSymbol {
            name: pou.name(db).text(db).to_string(),
            kind: SymbolKind::Pou(*pou),
        });
    });

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
