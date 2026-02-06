use auto_lsp::{
    default::db::file::File,
    lsp_types::{LocationLink, request::GotoImplementationResponse},
    salsa,
};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{pous::pou::Pou, semantic_index::semantic_index},
};

use crate::{handlers::ImplementationHandler, hir_node::HirNode};

impl<'db> HirNode<'db> {
    pub fn implementation(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        match self {
            HirNode::PouDecl(pou) => pou.implementation(db),
            _ => None,
        }
    }
}

impl<'db> ImplementationHandler<'db> for Pou<'db> {
    fn implementation(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        match self {
            Pou::Class(_) | Pou::Interface(_) => {
                let links = find_all_implementations(db, *self)
                    .iter()
                    .map(|pou| LocationLink {
                        target_uri: pou.get_scope_id(db).file(db).url(db).clone(),
                        target_range: pou.get_span(db).lsp(),
                        target_selection_range: pou.get_span(db).lsp(),
                        origin_selection_range: Some(self.get_span(db).lsp()),
                    })
                    .collect();

                Some(GotoImplementationResponse::Link(links))
            }
            _ => None,
        }
    }
}

// todo
// this could be optimized by filtering out files that do not contains the pou's name
// a custom symbol index could also be created for this purpose where only the references are stored
// this would be a lot more efficient for large workspaces
pub fn find_all_implementations<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> Vec<Pou<'db>> {
    let mut results = vec![];
    db.get_files().iter().for_each(|file| {
        results.extend(find_implementations(db, *file, pou));
    });
    results
}

#[salsa::tracked(returns(ref), no_eq)]
fn find_implementations<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    implemented: Pou<'db>,
) -> Vec<Pou<'db>> {
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
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    implemented: Pou<'db>,
    pous: &mut Vec<Pou<'db>>,
) {
    for candidate in pou.get_scope_id(db).inheritors(db).values() {
        if *candidate == implemented {
            pous.push(pou);
            return;
        }
    }
}
