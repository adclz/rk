use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName, HirNodeInfo,
    hir_ty::ty::CallableType,
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
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

    let mut candidates = vec![];
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        candidates.push(symbol.clone());
        std::ops::ControlFlow::Continue::<()>(())
    });

    if !candidates.is_empty() {
        let mut note = format!(
            "'{}' has parameter{} with similar name:\n",
            callable.get_name_ident(db).text(db),
            if candidates.len() > 1 { "s" } else { "" }
        );
        let display_count = candidates.len().min(5);

        for (i, candidate) in candidates.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.name));
        }

        if candidates.len() > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }
}
