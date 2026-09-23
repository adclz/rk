use std::iter::FusedIterator;
use std::sync::Arc;

use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::get_ast;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::hir_def::interned::namespace::NamespacePath;
use tracing::info_span;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::hir_def::config::ConfigDecl;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::pous::variable::LocatedAddress;
use crate::hir_def::program::ProgramDecl;
use crate::hir_def::scope::{Scope, ScopeId};
use index::{IndexVec, newtype_index};

/// Returns the semantic index of a given file
#[tracing::instrument(level = "debug", skip_all, name = "query_semantic_index")]
#[salsa::tracked(returns(ref), no_eq)]
pub fn semantic_index<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> SemanticIndex<'db> {
    let ast = info_span!("build AST").in_scope(|| get_ast(db, file));
    let root = match ast.get_root() {
        Some(root) => root,
        None => return SemanticIndex::empty(db, file, ast.nodes.clone()),
    };
    let source = match root.downcast_ref::<ast::generated::SourceFile>() {
        Some(source) => source,
        None => return SemanticIndex::empty(db, file, ast.nodes.clone()),
    };
    // A node the generated AST had no place for leaves ids pointing at the
    // wrong nodes, and the first cast panicked. The file is analyzed as
    // empty; its E0001 says why.
    if get_ast::accumulated::<auto_lsp::core::errors::ParseErrorAccumulator>(db, file)
        .iter()
        .any(|e| matches!(e.0, auto_lsp::core::errors::ParseError::AstError { .. }))
    {
        return SemanticIndex::empty(db, file, ast.nodes.clone());
    }

    SemanticIndexBuilder::new(db, file, get_ast(db, file), source).build()
}

#[newtype_index]
#[derive(Ord, PartialOrd, salsa::Update)]
pub struct NodeKey;

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct SemanticIndex<'db> {
    /// Global scope
    pub scope: ScopeId<'db>,

    pub(crate) file: File,

    /// The AST nodes of the file
    pub(crate) ast: Arc<Vec<Box<dyn AstNode>>>,

    /// Map of scope IDs to their corresponding scopes
    pub(crate) scopes: FxHashMap<usize, Arc<Scope<'db>>>,

    /// HIR nodes indexed by document order
    pub node_index: IndexVec<NodeKey, HirNode<'db>>,

    /// Program declarations in the file
    pub programs: Arc<Vec<ProgramDecl<'db>>>,

    /// Configuration declarations in the file
    pub configs: Arc<Vec<ConfigDecl<'db>>>,

    /// All namespace declarations in the file (flat, includes nested)
    pub namespaces: Arc<Vec<NamespaceDecl<'db>>>,
    /// The same declarations keyed by case-folded path, built once with the
    /// file, so a cross-file lookup is one probe per file rather than a fold
    /// and a compare per declaration.
    pub namespace_map: Arc<FxHashMap<NamespacePath, Vec<NamespaceDecl<'db>>>>,

    /// All *global* POU declarations in the file
    pub global_pous: Arc<Vec<Pou<'db>>>,

    /// Every I/O address the file mentions, with the declarations located
    /// at it: a CONFIGURATION's VAR_GLOBALs and a PROGRAM's VARs, in source
    /// order. An address only written bare in a body has none.
    pub located: Arc<
        std::collections::BTreeMap<
            LocatedAddress,
            Vec<crate::hir_def::pous::variable::VariableDecl<'db>>,
        >,
    >,

    /// A list of errors encountered during semantic analysis
    pub(crate) errors: Vec<IdeDiagnostic>,
}

impl<'db> SemanticIndex<'db> {
    pub fn empty(
        db: &'db dyn WorkspaceDataBase,
        file: File,
        ast: Arc<Vec<Box<dyn AstNode>>>,
    ) -> Self {
        let global_scope = ScopeId::global(db, file);
        let mut scopes = FxHashMap::default();
        let node_index = IndexVec::new();

        // Register the global scope so it can be looked up later
        let scope = Scope::new(
            file,
            crate::hir_def::scope::ScopeKind::Global,
            vec![],
            global_scope,
            None,
        );
        scopes.insert(global_scope.scope(db), Arc::new(scope));

        SemanticIndex {
            scope: global_scope,
            file,
            ast,
            scopes,
            node_index,
            programs: Arc::new(vec![]),
            configs: Arc::new(vec![]),
            namespaces: Arc::new(vec![]),
            namespace_map: Arc::new(FxHashMap::default()),
            global_pous: Arc::new(vec![]),
            located: Arc::new(Default::default()),
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
    pub fn errors(&self) -> &[IdeDiagnostic] {
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
