use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::pous::pou::{Pou, PouDecl},
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
};

#[derive(Default)]
pub struct FuzzyResult<'db> {
    pub pou: Vec<NamedSymbol<'db>>,
    pub variables: Vec<NamedSymbol<'db>>,
    pub struct_fields: Vec<NamedSymbol<'db>>,
}

pub fn fuzzy_pou_local_items<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
    query: &str,
) -> FuzzyResult<'db> {
    let mut results = FuzzyResult::default();
    let mut indexes = vec![];

    match pou.pou(db) {
        Pou::Function(f) => {
            f.variables(db).iter().for_each(|v| {
                indexes.push(NamedSymbol {
                    name: v.name(db).text(db).to_string(),
                    kind: SymbolKind::Variable(*v),
                })
            });
        }
        Pou::FunctionBlock(fb) => {
            fb.variables(db).iter().for_each(|v| {
                indexes.push(NamedSymbol {
                    name: v.name(db).text(db).to_string(),
                    kind: SymbolKind::Variable(*v),
                })
            });
        }
        _ => {}
    }

    let index = vec![SymbolIndex::create(db, indexes.into_boxed_slice())];

    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        match symbol.kind {
            SymbolKind::Variable(ty) => results.variables.push(symbol.clone()),
            SymbolKind::StructField(ty) => results.struct_fields.push(symbol.clone()),
            _ => results.pou.push(symbol.clone()),
        }

        std::ops::ControlFlow::Continue::<()>(())
    });

    results
}
