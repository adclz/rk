use auto_lsp::default::db::{BaseDatabase, file::File};

use crate::{
    hir_def::{
        pous::pou::{Pou, PouDecl},
        semantic_index::semantic_index,
    },
    hir_ty::ty::{Ty, TyKind, ty_for_pou},
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
    to_proto::ToProto,
};

// todo
// this could be optimized by filtering out files that do not contains the pou's name
// a custom symbol index could also be created for this purpose where only the references are stored
// this would be a lot more efficient for large workspaces
pub fn find_all_implementations<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Vec<PouDecl<'db>> {
    let mut results = vec![];
    let ty = ty_for_pou(db, pou);
    db.get_files().iter().for_each(|file| {
        results.extend(find_implementations(db, *file, ty));
    });
    results
}

#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref), no_eq)]
fn find_implementations<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    implemented: Ty<'db>,
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
    implemented: Ty<'db>,
    pous: &mut Vec<PouDecl<'db>>,
) {
    match ty_for_pou(db, pou).kind(db) {
        TyKind::Class {
            extends,
            implements,
            ..
        } => {
            if extends.is_some_and(|ext| ext.def(db).def_as_ty(db) == Some(implemented)) {
                pous.push(pou);
            } else if implements
                .iter()
                .find(|ipl| ipl.def(db).def_as_ty(db) == Some(implemented))
                .is_some()
            {
                pous.push(pou);
            }
        }
        TyKind::FunctionBlock { extends, .. } => {
            if extends.is_some_and(|ext| ext.def(db).def_as_ty(db) == Some(implemented)) {
                pous.push(pou);
            }
        }
        TyKind::Interface { implements, .. } => {
            if implements
                .iter()
                .find(|ipl| ipl.def(db).def_as_ty(db) == Some(implemented))
                .is_some()
            {
                pous.push(pou);
            }
        }
        _ => {}
    }
}
