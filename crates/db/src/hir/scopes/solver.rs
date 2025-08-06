use std::iter::FusedIterator;

use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::{
    check::errors::semantic_errors::{
        duplicate_using_declaration, namespace_already_in_scope, namespace_not_found,
    },
    hir::{
        interned::{
            identifier::Ident,
            namespace::{NamespaceAccess, NamespacePath},
        },
        namespace::Namespace,
        pous::pou::PouDecl,
        scopes::{
            iterators::AncestorsIter,
            scope::{ScopeId, ScopeKind},
        },
        semantic_index::{semantic_index, SemanticIndex},
        using::Using,
    },
};

/// Finds all namespaces that match the given path.
#[salsa::tracked(returns(ref), no_eq)]
pub fn shared_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    path: NamespacePath,
) -> Vec<Namespace<'db>> {
    db.get_files()
        .iter()
        .filter_map(|file| {
            semantic_index(db, *file).namespaces.iter().find_map(|ns| {
                if ns.path(db) == &path {
                    Some(*ns)
                } else {
                    None
                }
            })
        })
        .collect()
}

/// Finds all exported items in a given scope.
#[salsa::tracked(returns(ref))]
pub fn imported_pous_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: ScopeId,
) -> FxHashMap<Ident, PouDecl<'db>> {
    let sema = semantic_index(db, file);
    let scope = sema.get_scope(scope_id);

    let mut pous = FxHashMap::default();

    scope.usings.iter().for_each(|using| {
        let exported_namespaces = imported_namespaces(db, file, *using);

        for (path, ns) in exported_namespaces {
            for pou in ns.pous(db).iter() {
                pous.insert(*pou.name(db), *pou);
            }
        }
    });
    pous
}

// Rules for resolving Using directives

/// 1 - We first search if the namespace matches any of the namespaces declared in all files.
///
/// 2 - Then, we check if the namespace is not shadowed by any other namespace in the current scope.
///
/// 2.5 - Check if the directive is not declared multiple times in the same scope.
///
/// 3 - We then see if the visibility allows the namespace to be used in the current scope.
fn imported_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    using: Using<'db>,
) -> FxHashMap<NamespacePath, Namespace<'db>> {
    let mut results = FxHashMap::default();
    // 1: Check if the namespace exists
    let accross = shared_namespaces(db, using.path(db));

    if accross.is_empty() {
        namespace_not_found(db, using.span(db).clone(), using.path(db));
        return results;
    }

    let sema = semantic_index(db, file);

    // Check if this directive is not declared multiple times in the same scope
    sema.get_scope(using.scope_id(db))
        .usings
        .iter()
        .for_each(|u| {
            if u.path(db) == using.path(db) && *u != using {
                duplicate_using_declaration(db, file, using, *u);
            }
        });

    for scoped_ns in accross {
        let ns_file = scoped_ns.file(db);

        // 2: Check if the namespace is not shadowed by any other namespace in the current scope
        if ns_file == file {
            let mut ancestors = sema.ancestor_scopes(scoped_ns.scope_id(db));
            if let Some(parent) = ancestors.find_map(|sc| {
                if let ScopeKind::Namespace(ns_id) = sc.kind {
                    let ns_path = ns_id.path(db);
                    if *ns_path == using.path(db) {
                        Some(ns_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                namespace_already_in_scope(db, file, using, parent);
            }
        }
        results.insert(*scoped_ns.path(db), *scoped_ns);
    }
    results
}

pub fn resolve_namespace_access<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope: ScopeId,
    access: NamespaceAccess,
) -> Option<PouDecl<'db>> {
    let target = access.target(db);

    // Namespace is optional
    match access.namespace(db) {
        Some(ns) => {
            let namespaces = shared_namespaces(db, ns);
            namespaces.iter().find_map(|ns| {
                let pou = ns
                    .pous(db)
                    .iter()
                    .find(|pou| *pou.name(db) == target.ident)?;
                Some(*pou)
            })
        }
        None => semantic_index(db, file)
            .local_index(db, scope)
            .find_exact_pou(target.ident),
    }
}

// "The recursive call of POUs and methods is Implementer specific."
pub enum LocalSearchMode {
    Recursive,
    NonRecursive,
}

pub struct LocalIndex<'db> {
    db: &'db dyn BaseDatabase,
    sema: &'db SemanticIndex<'db>,
    scope: ScopeId,
    mode: LocalSearchMode,
}

impl<'db> LocalIndex<'db> {
    pub fn new(db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, scope: ScopeId) -> Self {
        Self {
            db,
            sema,
            scope,
            mode: LocalSearchMode::NonRecursive,
        }
    }

    pub fn allow_recursive(&mut self) {
        self.mode = LocalSearchMode::Recursive;
    }

    pub fn find_exact_pou(&self, pou_name: Ident) -> Option<PouDecl<'db>> {
        let pou = PouIterator::new(self.db, self.sema, self.scope)
            .find(|(name, _)| *name == pou_name)
            .map(|(_, pou)| pou)?;

        match self.mode {
            LocalSearchMode::Recursive => Some(pou),
            LocalSearchMode::NonRecursive => {
                let curr_scope = self.sema.get_scope(self.scope);
                match curr_scope.kind {
                    ScopeKind::Pou(pou_id) => {
                        if pou_id == pou {
                            None
                        } else {
                            Some(pou)
                        }
                    }
                    _ => Some(pou),
                }
            }
        }
    }

    pub fn list_pous(&self) -> PouIterator<'_> {
        PouIterator::new(self.db, self.sema, self.scope)
    }
}

/// An iterator that yields all POUs declared in a scope and its ancestors.
///
/// It first yields exported POUs from `using` statements, then POUs from the current scope,
/// and finally POUs from ancestor scopes.
pub struct PouIterator<'db> {
    db: &'db dyn BaseDatabase,
    sema: &'db SemanticIndex<'db>,

    exported_pous_iter: std::collections::hash_map::Iter<'db, Ident, PouDecl<'db>>,
    ancestor_iter: AncestorsIter<'db>,
    current_iterator: Option<std::slice::Iter<'db, PouDecl<'db>>>,
}

impl<'db> PouIterator<'db> {
    pub fn new(db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, scope: ScopeId) -> Self {
        let exported = imported_pous_in_scope(db, sema.file, scope);
        Self {
            db,
            sema,
            exported_pous_iter: exported.iter(),
            ancestor_iter: AncestorsIter::new(&sema.scopes, sema.get_scope(scope)),
            current_iterator: None,
        }
    }
}

impl<'db> Iterator for PouIterator<'db> {
    type Item = (Ident, PouDecl<'db>);

    fn next(&mut self) -> Option<Self::Item> {
        // Exported phase first
        if let Some((path, ns)) = self.exported_pous_iter.next() {
            return Some((*path, *ns));
        }

        if let Some(iter) = &mut self.current_iterator {
            if let Some(pou) = iter.next() {
                return Some((*pou.name(self.db), *pou));
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
                ScopeKind::Namespace(ns) => {
                    self.current_iterator = Some(ns.pous(self.db).iter());

                    if let Some(pou) = self.current_iterator.as_mut().unwrap().next() {
                        return Some((*pou.name(self.db), *pou));
                    }
                    // If the namespace has no POUs, continue to the next ancestor
                    self.current_iterator = None;
                }
                _ => continue,
            }
        }

        None
    }
}

impl FusedIterator for PouIterator<'_> {}
