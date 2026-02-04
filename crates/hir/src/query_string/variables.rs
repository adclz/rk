use db::WorkspaceDataBase;

use crate::{
    hir_def::scope::ScopeId,
    query_string::query::{NamedSymbol, SymbolIndex, SymbolKind},
};

#[salsa::tracked]
pub fn variable_symbol_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: ScopeId<'db>,
) -> SymbolIndex<'db> {
    let mut variables = vec![];
    pou.def_map(db).global_variables.iter().for_each(|(i, v)| {
        variables.push(NamedSymbol {
            name: i.text(db).to_string(),
            namespace: None,
            kind: SymbolKind::Variable(*v),
        });
    });

    SymbolIndex::create(db, variables.into_boxed_slice())
}
