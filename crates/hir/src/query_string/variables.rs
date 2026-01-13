use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName, HirNodeInfo,
    hir_def::{pous::pou::Pou, scope::ScopeId},
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
};

#[salsa::tracked]
pub fn variable_symbol_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: ScopeId<'db>,
) -> SymbolIndex<'db> {
    let mut variables = vec![];
    pou.def_map(db).global_variables.iter().for_each(|(i, v)| {
        variables.push(NamedSymbol {
            name: v.name(db).text(db).to_string(),
            namespace: None,
            kind: SymbolKind::Variable(*v),
        });
    });

    SymbolIndex::create(db, variables.into_boxed_slice())
}

pub fn fuzzy_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    diag: &mut IdeDiagnostic,
    query: &str,
) {
    let index = variable_symbol_index(db, pou.get_scope_id(db));

    let mut candidates = vec![];
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, &[index], |symbol| {
        candidates.push(symbol.clone());
        std::ops::ControlFlow::Continue::<()>(())
    });

    if !candidates.is_empty() {
        let mut note = format!(
            "'{}' has item{} with similar name:\n",
            pou.get_name_ident(db).text(db),
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
