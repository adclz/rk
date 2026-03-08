use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HasName,
    hir_def::expressions::spec::Struct,
    hir_ty::ty::{CallableType, Type},
    query_string::{method::method_symbol_index, query::{NamedSymbol, Query, SymbolIndex, SymbolKind}},
};

fn struct_symbol_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    strukt: Struct<'db>,
) -> Vec<SymbolIndex<'db>> {
    let mut struct_fields = vec![];
    strukt.elements(db).iter().for_each(|e| {
        struct_fields.push(NamedSymbol {
            name: e.name(db).text(db).to_string(),
            namespace: None,
            kind: SymbolKind::StructField(*e),
        });
    });

    vec![SymbolIndex::create(db, struct_fields.into_boxed_slice())]
}

/// Suggest fields with similar names for any type that has fields
/// (Struct, FunctionBlock, Class).
pub fn fuzzy_type_fields<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    diag: &mut IdeDiagnostic,
    query: &str,
) {
    let normalized = ty.normalize(db);
    match normalized {
        Type::Struct(strukt) => {
            fuzzy_suggest_from_index(
                db,
                &ty.type_name(db),
                "field",
                &struct_symbol_index(db, strukt),
                diag,
                query,
            );
        }
        Type::FunctionBlock(fb) => {
            let index = method_symbol_index(
                db,
                CallableType::FunctionBlock(fb),
            );
            fuzzy_suggest_from_index(db, fb.get_name_ident(db).text(db).as_str(), "field", index, diag, query);
        }
        Type::Class(c) => {
            let scope = c.scope_id(db);
            let def_map = scope.def_map(db);
            let mut symbols = vec![];
            for (_, v) in def_map.global_variables.iter() {
                symbols.push(NamedSymbol {
                    name: v.name(db).text(db).to_string(),
                    namespace: None,
                    kind: SymbolKind::Variable(*v),
                });
            }
            let index = vec![SymbolIndex::create(db, symbols.into_boxed_slice())];
            fuzzy_suggest_from_index(db, c.get_name_ident(db).text(db).as_str(), "field", &index, diag, query);
        }
        _ => {}
    }
}

pub fn fuzzy_suggest_from_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    type_name: &str,
    noun: &str,
    index: &[SymbolIndex<'db>],
    diag: &mut IdeDiagnostic,
    query: &str,
) {
    let mut candidates = vec![];
    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        candidates.push(symbol.clone());
        std::ops::ControlFlow::Continue::<()>(())
    });

    if !candidates.is_empty() {
        suggest_similar_note(type_name, noun, diag, candidates.iter().map(|c| c.name.as_str()));
    }
}

pub fn suggest_similar_note<'a>(
    owner_name: &str,
    noun: &str,
    diag: &mut IdeDiagnostic,
    candidates: impl Iterator<Item = &'a str>,
) {
    let collected: Vec<_> = candidates.take(6).collect();
    if collected.is_empty() {
        return;
    }

    let display_count = collected.len().min(5);
    let mut note = format!(
        "'{}' has {}{} with similar name:\n",
        owner_name,
        noun,
        if display_count > 1 { "s" } else { "" }
    );

    for (i, name) in collected.iter().take(display_count).enumerate() {
        if i > 0 {
            note.push('\n');
        }
        note.push_str(&format!("- {}", name));
    }

    if collected.len() > 5 {
        note.push_str("\n  ...");
    }

    diag.with_note(note);
}
