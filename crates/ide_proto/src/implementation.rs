use auto_lsp::{default::db::{BaseDatabase, file::File}, salsa};
use hir::hir_def::{pous::pou::PouDecl, semantic_index::semantic_index};

// todo
// this could be optimized by filtering out files that do not contains the pou's name
// a custom symbol index could also be created for this purpose where only the references are stored
// this would be a lot more efficient for large workspaces
pub fn find_all_implementations<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Vec<PouDecl<'db>> {
    let mut results = vec![];
    db.get_files().iter().for_each(|file| {
        results.extend(find_implementations(db, *file, pou));
    });
    results
}

#[salsa::tracked(returns(ref), no_eq)]
fn find_implementations<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    implemented: PouDecl<'db>,
) -> Vec<PouDecl<'db>> {
    let mut pous = vec![];
    let sema = semantic_index(db, file);

    sema.global_pous.iter().for_each(|pou| {
        check_implementations(db, *pou, implemented, &mut pous);
    });

    sema.namespaces.iter().for_each(|ns| {
        ns.pous(db).iter().for_each(|pou| {
            check_implementations(db, *pou, implemented, &mut pous);
        });
    });

    pous
}

fn check_implementations<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
    implemented: PouDecl<'db>,
    pous: &mut Vec<PouDecl<'db>>,
) {
    for candidate in pou.scope_id(db).inheritors(db) {
        if *candidate == implemented {
            pous.push(pou);
            return;
        }
    }
}
