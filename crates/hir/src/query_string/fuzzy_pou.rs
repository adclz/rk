use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    hir_def::{expressions::spec::Struct, pous::pou::{Pou, PouDecl}},
    hir_ty::{ty::{Ty, TyKind}},
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
};

#[salsa::tracked]
pub fn pou_symbol_index<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Vec<SymbolIndex<'db>> {
    let mut variables = vec![];
    pou.scope_id(db).global_variables(db).iter().for_each(|((i, v))| {
        variables.push(NamedSymbol {
            name: v.name(db).text(db).to_string(),
            kind: SymbolKind::Variable(*v),
        });
    });

    vec![SymbolIndex::create(db, variables.into_boxed_slice())]
}

pub fn fuzzy_pou_items<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
    diag: &mut IdeDiagnostic,
    query: &str,
){
    let index = pou_symbol_index(db, pou);

    let mut candidates = vec![];
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        candidates.push(symbol.clone());
        std::ops::ControlFlow::Continue::<()>(())
    });

    if !candidates.is_empty() {
        let mut note = "POU item(s) with similar name(s) exist:\n".to_string();
        let display_count = candidates.len().min(5);

        for (i, candidate) in candidates
            .iter()
            .take(display_count)
            .enumerate()
        {
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
