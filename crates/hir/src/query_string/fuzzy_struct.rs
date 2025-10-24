use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    hir_def::expressions::spec::Struct,
    hir_ty::ty::{Ty, TyKind},
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
};

#[salsa::tracked]
pub fn struct_symbol_index<'db>(
    db: &'db dyn BaseDatabase,
    strukt: Struct<'db>,
) -> Vec<SymbolIndex<'db>> {
    let mut struct_fields = vec![];
    strukt.elements(db).iter().for_each(|(e)| {
        struct_fields.push(NamedSymbol {
            name: e.name(db).text(db).to_string(),
            kind: SymbolKind::StructField(*e),
        });
    });

    vec![SymbolIndex::create(db, struct_fields.into_boxed_slice())]
}

pub fn fuzzy_struct_fields<'db>(
    db: &'db dyn BaseDatabase,
    strukt: Struct<'db>,
    diag: &mut IdeDiagnostic,
    query: &str,
){
    let index = struct_symbol_index(db, strukt);

    let mut candidates = vec![];
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        candidates.push(symbol.clone());
        std::ops::ControlFlow::Continue::<()>(())
    });

    if !candidates.is_empty() {
        let mut note = "STRUCT field(s) with similar name(s) exist:\n".to_string();
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
