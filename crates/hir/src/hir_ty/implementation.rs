use auto_lsp::default::db::{BaseDatabase, file::File};

use crate::{
    hir_def::{pous::pou::{Pou, PouDecl}, semantic_index::semantic_index},
    hir_ty::{name_res::resolve_namespace_access},
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
    db.get_files().iter().for_each(|file| {
        results.extend(find_implementations(db, *file, pou));
    });
    results
}

#[tracing::instrument(skip_all)]
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
    for candidate in pou.inheritors(db) {
        if candidate == implemented {
            pous.push(pou);
            return;
        }
    }
}

impl<'db> PouDecl<'db> {
    pub fn inheritors(&self, db: &'db dyn BaseDatabase) -> Vec<PouDecl<'db>> {
        match self.pou(db) {
            Pou::Class(class) => {
                let mut inheritors = vec![];
                if let Some(base) = class.extends(db)
                    && let Some(base) = resolve_namespace_access(db, base.scope_id, base.path)
                {
                    inheritors.push(base);
                }
                for iface in class.implements(db) {
                    resolve_namespace_access(db, iface.scope_id, iface.path).map(|iface| {
                        inheritors.push(iface);
                    });
                }
                inheritors
            }
            Pou::Interface(interface) => {
                let mut inheritors = vec![];
                if let Some(extends) = interface.extends(db) {
                    for iface in extends {
                        resolve_namespace_access(db, iface.scope_id, iface.path).map(|iface| {
                            inheritors.push(iface);
                        });
                    }
                }
                inheritors
            }
            Pou::FunctionBlock(fb) => {
                let mut inheritors = vec![];
                if let Some(base) = fb.extends(db)
                    && let Some(base) = resolve_namespace_access(db, base.scope_id, base.path)
                {
                    inheritors.push(base);
                }
                inheritors
            }
            _ => vec![],
        }
    }
}
