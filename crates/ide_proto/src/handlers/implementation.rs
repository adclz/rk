use auto_lsp::{
    default::db::file::File,
    lsp_types::{LocationLink, request::GotoImplementationResponse},
    salsa,
};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{hir_node::HirNode, pous::pou::Pou, semantic_index::semantic_index},
};

use crate::handlers::ImplementationHandler;

impl<'db> ImplementationHandler<'db> for HirNode<'db> {
    fn implementation(&self, db: &'db dyn WorkspaceDataBase) -> Option<GotoImplementationResponse> {
        match self {
            HirNode::PouDecl(pou) => pou.implementation(db),
            // A prototype's implementations are the same-named methods of
            // every POU implementing its interface. Only the interface itself
            // answered, so asking on the method got nothing.
            HirNode::MethodRef(method) => method.implementation(db),
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
                        target_range: hir::denormalize(
                            db,
                            pou.get_scope_id(db).file(db),
                            &pou.get_span(db),
                        )
                        .unwrap_or_default(),
                        target_selection_range: hir::denormalize(
                            db,
                            pou.get_scope_id(db).file(db),
                            &pou.get_span(db),
                        )
                        .unwrap_or_default(),
                        origin_selection_range: Some(
                            hir::denormalize(
                                db,
                                self.get_scope_id(db).file(db),
                                &self.get_span(db),
                            )
                            .unwrap_or_default(),
                        ),
                    })
                    .collect();

                Some(GotoImplementationResponse::Link(links))
            }
            _ => None,
        }
    }
}

impl<'db> ImplementationHandler<'db> for hir::hir_ty::head::inheritance::MethodRef<'db> {
    fn implementation(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        use hir::HasName;

        let owner = owner_of(db, *self)?;
        let name = self.get_name_ident(db);

        let links: Vec<LocationLink> = find_all_implementations(db, owner)
            .iter()
            .filter_map(|pou| {
                let scope = pou.get_scope_id(db);
                scope
                    .method_declarations(db)?
                    .iter()
                    .find(|declared| declared.get_name_ident(db) == name)
                    .copied()
            })
            .map(|declared| {
                let file = declared.get_scope_id(db).file(db);
                let range =
                    hir::denormalize(db, file, &declared.get_name_span(db)).unwrap_or_default();
                LocationLink {
                    target_uri: file.url(db).clone(),
                    target_range: range,
                    target_selection_range: range,
                    origin_selection_range: hir::denormalize(
                        db,
                        self.get_scope_id(db).file(db),
                        &self.get_name_span(db),
                    ),
                }
            })
            .collect();

        (!links.is_empty()).then_some(GotoImplementationResponse::Link(links))
    }
}

/// The POU a method belongs to. A method's scope does not name it: a
/// prototype's parent scope is the file's, so the owner is found by asking
/// each POU in the file whether the method is one of its own.
fn owner_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    method: hir::hir_ty::head::inheritance::MethodRef<'db>,
) -> Option<Pou<'db>> {
    use hir::hir_ty::head::inheritance::MethodRef;

    let sema = semantic_index(db, method.get_scope_id(db).file(db));
    let owns = |pou: &Pou<'db>| {
        let scope = pou.get_scope_id(db);
        match method {
            MethodRef::Declared(declared) => scope
                .method_declarations(db)
                .is_some_and(|methods| methods.contains(&declared)),
            MethodRef::Prototype(prototype) => scope
                .method_prototypes(db)
                .is_some_and(|methods| methods.contains(&prototype)),
        }
    };

    sema.global_pous
        .iter()
        .chain(sema.namespaces.iter().flat_map(|ns| ns.pous(db).iter()))
        .find(|pou| owns(pou))
        .copied()
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
