use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName, HirNodeInfo,
    hir_ty::ty::CallableType,
    query_string::{
        fields::fuzzy_suggest_from_index,
        query::{NamedSymbol, SymbolIndex, SymbolKind},
    },
};

#[salsa::tracked(returns(ref))]
pub fn method_symbol_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
) -> Vec<SymbolIndex<'db>> {
    let mut variables = vec![];
    callable
        .get_scope_id(db)
        .def_map(db)
        .local_variables
        .iter()
        .for_each(|(i, v)| {
            variables.push(NamedSymbol {
                name: v.name(db).text(db).to_string(),
                namespace: None,
                kind: SymbolKind::Variable(*v),
            });
        });

    vec![SymbolIndex::create(db, variables.into_boxed_slice())]
}

pub fn fuzzy_callable_type_parameters<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
    diag: &mut IdeDiagnostic,
    query: &str,
) {
    let index = method_symbol_index(db, callable);
    fuzzy_suggest_from_index(
        db,
        callable.get_name_ident(db).text(db).as_str(),
        "parameter",
        index,
        diag,
        query,
    );
}
