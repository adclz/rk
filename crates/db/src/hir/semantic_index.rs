use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scopes::iterators::AncestorsIter;
use crate::hir::scopes::scope::{
    NamespaceId, PouId, Scope, ScopeId, ScopedNamespaceId, ScopedPouId,
};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::to_proto::{IterToProto, ToProto};

/// Returns the semantic index of a given file
#[salsa::tracked]
pub fn semantic_index<'db>(db: &'db dyn BaseDatabase, file: File) -> Option<SemanticIndex<'db>> {
    let ast = get_ast(db, file).get_root()?;
    let source = ast.downcast_ref::<ast::generated::SourceFile>()?;

    Some(SemanticIndexBuilder::new(db, file, source).build())
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

    /// Map of scope IDs to their containing POUs
    pub scope_to_pous: FxHashMap<ScopeId, FxHashMap<Ident, ScopedPouId>>,
}

impl<'db> SemanticIndex<'db> {
    pub fn get_namespace(&'db self, key: NamespaceId) -> &'db Namespace<'db> {
        &self.namespace_keys[&key]
    }

    pub fn get_pou(&'db self, key: PouId) -> &'db PouDecl<'db> {
        &self.pou_keys[&key]
    }

    pub fn get_scope(&'db self, id: ScopeId) -> &'db Scope<'db> {
        &self.scopes[&id]
    }

    pub(crate) fn ancestor_scopes(&self, scope: ScopeId) -> AncestorsIter {
        AncestorsIter::new(&self.scopes, self.get_scope(scope))
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
