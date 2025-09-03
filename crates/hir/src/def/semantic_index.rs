use std::iter::FusedIterator;
use std::ops::ControlFlow;
use std::sync::Arc;

use auto_lsp::core::ast::AstNode;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::default::db::{BaseDatabase, file::File};
use rustc_hash::FxHashMap;
use tracing::info_span;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::sem_errors::AnalysisError;
use crate::def::interned::identifier::Ident;
use crate::def::namespace::NamespaceDecl;
use crate::def::pous::pou::PouDecl;
use crate::def::scope::{FileScopeId, Scope};
use crate::def::using::Using;
use crate::to_proto::ToProto;
use crate::ty::expr_resolver::ResolvedExpr;
use crate::ty::name_res::pous_in_scope;
use crate::ty::stmt_resolver::ResolvedStmt;
use crate::ty::ty::Ty;
use crate::ty::ty_path_expr_resolver::ResolvedPathResult;
use crate::ty::ty_var_access_resolver::ResolvedVarResult;
use crate::walk::WalkHir;

/// Returns the semantic index of a given file
#[tracing::instrument(skip_all, name = "query_semantic_index")]
#[salsa::tracked(returns(ref), no_eq)]
pub fn semantic_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SemanticIndex<'db> {
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
    pub(crate) scopes: FxHashMap<FileScopeId<'db>, Scope<'db>>,

    /// All *global* POU declarations in the file
    pub global_pous: Vec<PouDecl<'db>>,

    /// All namespaces in the file
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
            global_pous: vec![],
            namespaces: vec![],
            errors: vec![],
        }
    }

    /// Get the scope corresponding to the given ID.
    ///
    /// Panics if the scope does not belong to the same file as the semantic index.
    pub fn get_scope(
        &'db self,
        db: &'db dyn BaseDatabase,
        id: FileScopeId<'db>,
    ) -> &'db Scope<'db> {
        assert!(self.file == id.file(db));
        &self.scopes[&id]
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
        scope: FileScopeId<'db>,
    ) -> &'db FxHashMap<Ident, PouDecl<'db>> {
        pous_in_scope(db, scope)
    }

    /// Returns a [`ScopeIterator`] starting from the given scope.
    pub fn scope_iterator(
        &self,
        db: &'db dyn BaseDatabase,
        scope: FileScopeId<'db>,
    ) -> ScopeIterator {
        ScopeIterator::new(&self.scopes, self.get_scope(db, scope))
    }

    /// Returns all errors encountered during semantic analysis.
    pub fn errors(&self) -> &[AnalysisError<'db>] {
        &self.errors
    }
}

/// Iterator over scopes in a given scope hierarchy
pub struct ScopeIterator<'db> {
    scopes: &'db FxHashMap<FileScopeId<'db>, Scope<'db>>,
    next_id: Option<FileScopeId<'db>>,
}

impl<'db> ScopeIterator<'db> {
    pub fn new(
        scopes: &'db FxHashMap<FileScopeId<'db>, Scope<'db>>,
        scope: &'db Scope<'db>,
    ) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum HirNode<'db> {
    Namespace(NamespaceDecl<'db>),
    Using(Using<'db>),
    Ty(Ty<'db>),
    ResolvedPathResult(ResolvedPathResult<'db>),
    ResolvedVarResult(ResolvedVarResult<'db>),
    ResolvedStmt(ResolvedStmt<'db>),
    ResolvedExpr(ResolvedExpr<'db>),
}

impl<'db> HirNode<'db> {
    pub fn as_proto(&'db self) -> &'db dyn ToProto<'db> {
        match self {
            HirNode::Namespace(n) => n,
            HirNode::Using(u) => u,
            HirNode::Ty(t) => t,
            HirNode::ResolvedPathResult(p) => p,
            HirNode::ResolvedVarResult(v) => v,
            HirNode::ResolvedStmt(s) => s,
            HirNode::ResolvedExpr(e) => e,
        }
    }

    pub fn get_span(&'db self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            HirNode::Namespace(n) => n.get_span(db),
            HirNode::Using(u) => u.get_span(db),
            HirNode::Ty(t) => t.get_span(db),
            HirNode::ResolvedPathResult(p) => p.get_span(db),
            HirNode::ResolvedVarResult(v) => v.get_span(db),
            HirNode::ResolvedStmt(s) => s.get_span(db),
            HirNode::ResolvedExpr(e) => e.get_span(db),
        }
    }
}

impl<'db> SemanticIndex<'db> {
    #[tracing::instrument(skip(self, db))]
    pub fn descendant_at(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<HirNode<'db>> {
        let mut best_match: Option<HirNode<'db>> = None;

        let _ = self.walk_hir(db, &mut |node| {
            let range = node.get_span(db);
            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Always update the best match when we find a containing node
                // This ensures we get the deepest (last visited) node in the tree
                best_match = Some(node);
            }
            ControlFlow::Continue(())
        });

        best_match
    }
}
