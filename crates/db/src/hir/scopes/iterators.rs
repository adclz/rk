use std::iter::FusedIterator;

use rustc_hash::FxHashMap;

use crate::hir::scopes::scope::{Scope, ScopeId};

pub struct AncestorsIter<'db> {
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
