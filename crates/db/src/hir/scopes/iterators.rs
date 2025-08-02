use std::iter::FusedIterator;

use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir::{
    interned::{identifier::Ident, namespace::NamespacePath},
    scopes::{
        scope::{PouId, Scope, ScopeId, ScopeKind, ScopedNamespaceId, ScopedPouId},
        solver::exported_items_in_scope,
    },
    semantic_index::SemanticIndex,
};

pub(crate) struct AncestorsIter<'db> {
    scopes: &'db FxHashMap<ScopeId, Scope<'db>>,
    next_id: Option<ScopeId>,
}

impl<'db> AncestorsIter<'db> {
    pub fn new(scopes: &'db FxHashMap<ScopeId, Scope<'db>>, scope: &'db Scope<'db>) -> Self {
        Self {
            scopes,
            next_id: Some(scope.id),
        }
    }
}

impl<'db> Iterator for AncestorsIter<'db> {
    type Item = &'db Scope<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next_id?;
        let current = self.scopes.get(&current)?;
        self.next_id = current.parent;
        Some(current)
    }
}

impl FusedIterator for AncestorsIter<'_> {}

#[derive(Default, Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ScopedMap {
    pub namespaces: FxHashMap<NamespacePath, ScopedNamespaceId>,
    pub pous: FxHashMap<Ident, ScopedPouId>,
}

/// An iterator that yields all POUs declared in a scope and its ancestors.
///
/// It first yields exported POUs from `using` statements, then POUs from the current scope,
/// and finally POUs from ancestor scopes.
pub struct PouIterator<'db> {
    db: &'db dyn BaseDatabase,
    sema: &'db SemanticIndex<'db>,

    exported_pous_iter: std::collections::hash_map::Iter<'db, Ident, ScopedPouId>,
    ancestor_iter: AncestorsIter<'db>,
    current_iterator: Option<std::slice::Iter<'db, PouId>>,
}

impl<'db> PouIterator<'db> {
    pub fn new(db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, scope: ScopeId) -> Self {
        let exported = exported_items_in_scope(db, sema.file, scope);
        Self {
            db,
            sema,
            exported_pous_iter: exported.pous.iter(),
            ancestor_iter: AncestorsIter::new(&sema.scopes, sema.get_scope(scope)),
            current_iterator: None,
        }
    }
}

impl<'db> Iterator for PouIterator<'db> {
    type Item = (Ident, ScopedPouId);

    fn next(&mut self) -> Option<Self::Item> {
        // Exported phase first
        if let Some((path, ns)) = self.exported_pous_iter.next() {
            return Some((*path, *ns));
        }

        if let Some(iter) = &mut self.current_iterator {
            if let Some(pou) = iter.next() {
                let pou_name = self.sema.pou_keys[pou].name(self.db);
                return Some((*pou_name, ScopedPouId(*pou, self.sema.file)));
            } else {
                self.current_iterator = None;
            }
        }

        // Fallback to ancestor declarations
        while let Some(scope) = self.ancestor_iter.next() {
            match scope.kind {
                ScopeKind::Global => {
                    // todo:
                    return None;
                }
                ScopeKind::Namespace(ns_id) => {
                    let ns = self.sema.get_namespace(ns_id);
                    self.current_iterator = Some(ns.pous(self.db).iter());

                    return self.current_iterator.as_mut().unwrap().next().map(|pou| {
                        let pou_name = self.sema.pou_keys[pou].name(self.db);
                        (*pou_name, ScopedPouId(*pou, self.sema.file))
                    });
                }
                _ => continue,
            }
        }

        None
    }
}

impl FusedIterator for PouIterator<'_> {}
