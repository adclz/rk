use std::iter::FusedIterator;

use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;
use tracing::info_span;

use crate::hir::interned::identifier::Ident;
use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scopes::scope::{FileScopeId, Scope};
use crate::hir::scopes::solver::pous_in_scope;
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::to_proto::{IterToProto, ToProto};

/// Returns the semantic index of a given file
#[tracing::instrument(skip_all, name = "query_semantic_index")]
#[salsa::tracked(returns(ref))]
pub fn semantic_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SemanticIndex<'db> {
    let ast = match info_span!("build AST").in_scope(|| get_ast(db, file).get_root()) {
        Some(ast) => ast,
        None => return SemanticIndex::empty(file),
    };

    let source = match ast.downcast_ref::<ast::generated::SourceFile>() {
        Some(source) => source,
        None => return SemanticIndex::empty(file),
    };

    SemanticIndexBuilder::new(db, file, source).build()
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct SemanticIndex<'db> {
    pub file: File,

    /// Map of scope IDs to their corresponding scopes
    pub scopes: FxHashMap<FileScopeId, Scope<'db>>,

    /// Global POU declarations in the file
    pub global_pous: Vec<PouDecl<'db>>,

    /// All namespaces in the file
    pub namespaces: Vec<Namespace<'db>>,
}

impl<'db> SemanticIndex<'db> {
    pub fn empty(file: File) -> Self {
        SemanticIndex {
            file,
            scopes: FxHashMap::default(),
            global_pous: Vec::new(),
            namespaces: Vec::new(),
        }
    }

    pub fn get_scope(&'db self, id: FileScopeId) -> &'db Scope<'db> {
        &self.scopes[&id]
    }

    /// Returns a [`ScopeIterator`] starting from the given scope.
    pub fn scope_iterator(&self, scope: FileScopeId) -> ScopeIterator {
        ScopeIterator::new(&self.scopes, self.get_scope(scope))
    }

    /// Returns all POUs available in a given scope
    ///
    /// This includes:
    /// - Locally declared POUs
    /// - Imported POUs via USING directives
    /// - Inherited POUs from ancestor scopes (including the global scope)
    pub fn pous_in_scope(
        &'db self,
        db: &'db dyn BaseDatabase,
        scope: FileScopeId,
    ) -> &'db FxHashMap<Ident, PouDecl<'db>> {
        pous_in_scope(db, self.file, scope)
    }
}

impl<'db> IterToProto<'db> for SemanticIndex<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.namespaces.iter().flat_map(move |ns| ns.iter(db, sema))
    }
}

/// Iterator over scopes in a given scope hierarchy
pub struct ScopeIterator<'db> {
    scopes: &'db FxHashMap<FileScopeId, Scope<'db>>,
    next_id: Option<FileScopeId>,
}

impl<'db> ScopeIterator<'db> {
    pub fn new(scopes: &'db FxHashMap<FileScopeId, Scope<'db>>, scope: &'db Scope<'db>) -> Self {
        Self {
            scopes,
            next_id: Some(scope.id),
        }
    }
}

impl<'db> Iterator for ScopeIterator<'db> {
    type Item = &'db Scope<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next_id?;
        let current = self.scopes.get(&current)?;
        self.next_id = current.parent;
        Some(current)
    }
}

impl FusedIterator for ScopeIterator<'_> {}
