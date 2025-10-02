use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::recovery::pou::FuzzyResult,
    hir_def::pous::pou::{Pou, PouDecl},
    hir_ty::ty::ty_for_variable,
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
};

pub fn fuzzy_func_local_items<'db>(
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
                    kind: SymbolKind::Variable(ty_for_variable(db, *v)),
                })
            });
        }
        Pou::FunctionBlock(fb) => {
            fb.variables(db).iter().for_each(|v| {
                indexes.push(NamedSymbol {
                    name: v.name(db).text(db).to_string(),
                    kind: SymbolKind::Variable(ty_for_variable(db, *v)),
                })
            });
        }
        _ => {}
    }

    let index = vec![SymbolIndex::create(db, indexes.into_boxed_slice())];

    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        if let SymbolKind::Variable(ty) = symbol.kind
            && (ty.is_variable_input(db) || ty.is_variable_inout(db) || ty.is_variable_output(db))
        {
            results.variables.push(symbol.clone())
        }

        std::ops::ControlFlow::Continue::<()>(())
    });

    results
}
