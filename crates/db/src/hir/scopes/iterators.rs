use std::iter::FusedIterator;

use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir::{
    interned::{identifier::Ident, namespace::NamespacePath},
    scopes::{
        scope::{Scope, ScopeId, ScopeKind},
        solver::imported_pous_in_scope,
    },
    semantic_index::SemanticIndex,
};

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
