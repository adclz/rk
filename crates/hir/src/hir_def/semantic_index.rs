use std::iter::FusedIterator;
use std::sync::Arc;

use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::get_ast;
use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;
use tracing::info_span;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::analysis_error::AnalysisError;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::program::ProgramDecl;
use crate::hir_def::scope::{Scope, ScopeId};

/// Returns the semantic index of a given file
#[tracing::instrument(skip_all, name = "query_semantic_index")]
#[salsa::tracked(returns(ref), no_eq)]
pub fn semantic_index<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> SemanticIndex<'db> {
    let ast = info_span!("build AST").in_scope(|| get_ast(db, file));
    let root = match ast.get_root() {
        Some(root) => root,
        None => return SemanticIndex::empty(file, ast.nodes.clone()),
    };
    let source = match root.downcast_ref::<ast::generated::SourceFile>() {
        Some(source) => source,
        None => return SemanticIndex::empty(file, ast.nodes.clone()),
    };

    SemanticIndexBuilder::new(db, file, get_ast(db, file), source).build()
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct SemanticIndex<'db> {
    pub(crate) file: File,

    /// The AST nodes of the file
    pub(crate) ast: Arc<Vec<Box<dyn AstNode>>>,

    /// Map of scope IDs to their corresponding scopes
    pub(crate) scopes: FxHashMap<usize, Arc<Scope<'db>>>,

    /// Program declarations in the file
    pub(crate) programs: Vec<ProgramDecl<'db>>,

    /// All *global* namespaces in the file
    pub global_namespaces: Vec<NamespaceDecl<'db>>,

    /// All *global* POU declarations in the file
    pub global_pous: Vec<Pou<'db>>,

    /// All  namespaces in the file
    pub namespaces: Vec<NamespaceDecl<'db>>,

    /// A list of errors encountered during semantic analysis
    pub(crate) errors: Vec<AnalysisError<'db>>,
}

impl<'db> SemanticIndex<'db> {
    pub fn empty(file: File, ast: Arc<Vec<Box<dyn AstNode>>>) -> Self {
        SemanticIndex {
            file,
            ast,
            scopes: FxHashMap::default(),
            programs: vec![],
            global_namespaces: vec![],
            global_pous: vec![],
            namespaces: vec![],
            errors: vec![],
        }
    }

    /// Returns a [`ScopeIterator`] starting from the given scope.
    pub fn scope_iterator(
        &self,
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
    ) -> ScopeIterator<'_> {
        ScopeIterator::new(db, &self.scopes, &scope)
    }

    /// Returns all errors encountered during semantic analysis.
    pub fn errors(&self) -> &[AnalysisError<'db>] {
        &self.errors
    }
}

/// Get the scope corresponding to the given ID.
///
/// Panics if the scope does not belong to the same file as the semantic index.
#[salsa::tracked(returns(deref))]
pub fn get_scope<'db>(db: &'db dyn WorkspaceDataBase, id: ScopeId<'db>) -> Arc<Scope<'db>> {
    let sema = semantic_index(db, id.file(db));
    Arc::clone(&sema.scopes[&id.scope(db)])
}

/// Iterator over scopes in a given scope hierarchy
pub struct ScopeIterator<'db> {
    db: &'db dyn WorkspaceDataBase,
    scopes: &'db FxHashMap<usize, Arc<Scope<'db>>>,
    next_id: Option<ScopeId<'db>>,
}

impl<'db> ScopeIterator<'db> {
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        scopes: &'db FxHashMap<usize, Arc<Scope<'db>>>,
        scope: &ScopeId<'db>,
    ) -> Self {
        Self {
            db,
            scopes,
            next_id: Some(*scope),
        }
    }
}

impl<'db> Iterator for ScopeIterator<'db> {
    type Item = &'db Scope<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next_id?;
        let current = self.scopes.get(&current.scope(self.db))?;
        self.next_id = current.parent;
        Some(current)
    }
}

impl FusedIterator for ScopeIterator<'_> {}
