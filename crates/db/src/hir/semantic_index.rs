use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scopes::iterators::{AncestorsIter};
use crate::hir::scopes::scope::{
    NamespaceId, PouId, Scope, ScopeId, ScopedNamespaceId, FilePouId,
};
use crate::hir::scopes::solver::{imported_pous_in_scope, LocalIndex};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::to_proto::{IterToProto, ToProto};

/// Returns the semantic index of a given file
#[salsa::tracked]
pub fn semantic_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SemanticIndex<'db> {
    let ast = match get_ast(db, file).get_root() {
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
    pub scopes: FxHashMap<ScopeId, Scope<'db>>,

    /// Map of namespace keys to their corresponding namespaces
    pub namespace_keys: FxHashMap<NamespaceId, Namespace<'db>>,

    /// Map of pou keys to their corresponding POU declarations
    pub pou_keys: FxHashMap<PouId, PouDecl<'db>>,

    /// Map of scope IDs to their containing namespaces
    pub scope_to_namespaces: FxHashMap<ScopeId, FxHashMap<NamespacePath, ScopedNamespaceId>>,
}

impl<'db> SemanticIndex<'db> {
    pub fn empty(file: File) -> Self {
        SemanticIndex {
            file,
            scopes: FxHashMap::default(),
            namespace_keys: FxHashMap::default(),
            pou_keys: FxHashMap::default(),
            scope_to_namespaces: FxHashMap::default()
        }
    }

    pub fn get_namespace(&'db self, key: NamespaceId) -> &'db Namespace<'db> {
        &self.namespace_keys[&key]
    }

    pub fn get_pou(&'db self, key: PouId) -> &'db PouDecl<'db> {
        &self.pou_keys[&key]
    }

    pub fn get_scope(&'db self, id: ScopeId) -> &'db Scope<'db> {
        &self.scopes[&id]
    }

    pub fn ancestor_scopes(&self, scope: ScopeId) -> AncestorsIter {
        AncestorsIter::new(&self.scopes, self.get_scope(scope))
    }

    pub fn local_index(&self, db: &'db dyn BaseDatabase, scope: ScopeId) -> LocalIndex {
        LocalIndex::new(db, self, scope)
    }
}

impl<'db> IterToProto<'db> for SemanticIndex<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.namespace_keys
            .iter()
            .flat_map(move |(key, ns)| ns.iter(db, sema))
    }
}
