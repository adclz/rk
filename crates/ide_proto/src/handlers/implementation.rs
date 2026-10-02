// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::{LocationLink, request::GotoImplementationResponse};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{hir_node::HirNode, pous::pou::Pou, semantic_index::semantic_index},
    hir_ty::oop::descendants,
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
            Pou::FunctionBlock(_) | Pou::Class(_) | Pou::Interface(_) => {
                let links = descendants(db, *self)
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

impl<'db> ImplementationHandler<'db> for hir::hir_ty::oop::MethodRef<'db> {
    fn implementation(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        use hir::HasName;

        let owner = owner_of(db, *self)?;
        // Each implementer's own method of that name, in whatever case it
        // declares it: `START` implements `Start`.
        let name = self.get_name_ident(db);
        let links: Vec<LocationLink> = descendants(db, owner)
            .iter()
            .filter_map(|pou| {
                pou.get_scope_id(db)
                    .def_map(db)
                    .declared_methods
                    .get(&name)
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
    method: hir::hir_ty::oop::MethodRef<'db>,
) -> Option<Pou<'db>> {
    use hir::hir_ty::oop::MethodRef;

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
