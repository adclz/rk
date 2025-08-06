use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scopes::iterators::AncestorsIter;
use crate::hir::scopes::scope::{Scope, ScopeId};
use crate::hir::scopes::solver::LocalIndex;
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::to_proto::{IterToProto, ToProto};

/// Returns the semantic index of a given file
#[salsa::tracked(returns(ref))]
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

    pub fn get_scope(&'db self, id: ScopeId) -> &'db Scope<'db> {
        &self.scopes[&id]
    }

    pub fn ancestor_scopes(&self, scope: ScopeId) -> AncestorsIter {
        AncestorsIter::new(&self.scopes, self.get_scope(scope))
    }

    pub fn local_index(&'db self, db: &'db dyn BaseDatabase, scope: ScopeId) -> LocalIndex<'db> {
        LocalIndex::new(db, self, scope)
    }
}

impl<'db> IterToProto<'db> for SemanticIndex<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.namespaces
            .iter()
            .flat_map(move |ns| ns.iter(db, sema))
    }
}
