use std::iter::FusedIterator;

use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir::{interned::{identifier::Ident, namespace::NamespacePath}, scopes::{scope::{Scope, ScopeId, ScopeKind, ScopedNamespaceId, ScopedPouId}, solver::exported_items_in_scope}, semantic_index::SemanticIndex};

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

pub struct PouIterator<'db> {
    db: &'db dyn BaseDatabase,
    sema: &'db SemanticIndex<'db>,

    exported_pous_iter: std::collections::hash_map::Iter<'db, Ident, ScopedPouId>,
    ancestor_iter: AncestorsIter<'db>,
}

impl<'db> PouIterator<'db> {
    pub fn new(db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, scope: ScopeId) -> Self {
        let exported = exported_items_in_scope(db, sema.file, sema.get_scope(scope));
        Self {
            db,
            sema,
            exported_pous_iter: exported.pous.iter(),
            ancestor_iter: AncestorsIter::new(&sema.scopes, sema.get_scope(scope)),
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

        // Fallback to ancestor declarations
        while let Some(scope) = self.ancestor_iter.next() {
            if let ScopeKind::Pou(pou_id) = scope.kind {
                return Some((*self.sema.pou_keys[&pou_id].name(self.db), ScopedPouId(pou_id, scope.file)))
            } else {
                continue;
            }
        }

        None
    }
}

impl FusedIterator for PouIterator<'_> {}
