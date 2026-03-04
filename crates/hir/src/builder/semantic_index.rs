use std::sync::Arc;

use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::ParsedAst;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use index::IndexVec;
use rustc_hash::FxHashMap;

use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::config::ConfigDecl;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::program::ProgramDecl;
use crate::hir_def::scope::{Scope, ScopeId, ScopeKind};
use crate::hir_def::semantic_index::{NodeKey, SemanticIndex};
use crate::hir_def::using::Using;
use crate::{AstId, HirNodeInfo, Visibility};

pub struct SemanticIndexBuilder<'db> {
    pub(crate) source: &'db ast::generated::SourceFile,
    pub(crate) ast: &'db ParsedAst,

    pub(crate) db: &'db dyn WorkspaceDataBase,
    pub(crate) file: File,

    /// The current scope ID being processed (by default, the global scope).
    pub(crate) current_scope: ScopeId<'db>,

    /// Maps scope IDs to their corresponding scopes.
    pub(crate) scope_keys: FxHashMap<usize, Arc<Scope<'db>>>,

    /// HIR nodes indexed by document order.
    pub(crate) node_index: IndexVec<NodeKey, HirNode<'db>>,

    pub(crate) programs: Vec<ProgramDecl<'db>>,
    pub(crate) configs: Vec<ConfigDecl<'db>>,
    pub(crate) global_namespaces: Vec<NamespaceDecl<'db>>,
    pub(crate) global_pous: Vec<Pou<'db>>,

    /// Maps scope IDs to their corresponding namespaces.
    pub(crate) namespaces: Vec<NamespaceDecl<'db>>,

    /// Counter for generating stable scope IDs.
    ///
    /// The reason for having a separate counter is that each scope will trigger a recomputation if it's ID changes.
    /// If we use the AST id directly, then any changes in the previous nodes will cause all subsequent scopes to be recomputed.
    ///
    /// Since scope are only created when visiting a Pou or Namespace,
    /// writing variables / statements / expressions, will preserve the IDs of scopes.
    pub(crate) scope_ctr: usize,

    pub(crate) errors: Vec<IdeDiagnostic>,
}

impl<'db> SemanticIndexBuilder<'db> {
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        file: File,
        ast: &'db ParsedAst,
        source: &'db ast::generated::SourceFile,
    ) -> Self {
        Self {
            db,
            file,
            ast,
            source,
            scope_keys: FxHashMap::default(),
            node_index: IndexVec::new(),
            programs: vec![],
            configs: vec![],
            global_namespaces: vec![],
            global_pous: vec![],
            namespaces: vec![],
            scope_ctr: 0,
            current_scope: ScopeId::global(db, file),
            errors: vec![],
        }
    }

    pub fn generate_scope_id(&mut self) -> ScopeId<'db> {
        let scope_id = ScopeId::new(self.db, self.file, self.scope_ctr);
        self.scope_ctr += 1;
        scope_id
    }

    /// Try a fallible parse, collecting the error and returning `None` on failure.
    pub fn try_parse<T>(&mut self, result: Result<T, IdeDiagnostic>) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                self.errors.push(e);
                None
            }
        }
    }

    /// Try a fallible parse, collecting the error and returning `T::default()` on failure.
    pub fn parse_or_default<T: Default>(&mut self, result: Result<T, IdeDiagnostic>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => {
                self.errors.push(e);
                T::default()
            }
        }
    }

    /// Register a HIR node in the node index, keyed by its AstId.
    pub fn register_node(&mut self, _id: AstId, node: HirNode<'db>) {
        self.node_index.push(node);
    }

    /// Create a VariableDecl, register it in the node index, and return it.
    pub fn new_variable(
        &mut self,
        name: crate::hir_def::interned::identifier::Ident,
        name_id: crate::AstId,
        kind: crate::hir_def::pous::variable::VariableKind,
        variadic: bool,
        spec: crate::hir_def::expressions::spec::Spec<'db>,
        init: Option<crate::hir_def::expressions::expression::InitExpr<'db>>,
        id: crate::AstId,
        scope_id: crate::hir_def::scope::ScopeId<'db>,
    ) -> crate::hir_def::pous::variable::VariableDecl<'db> {
        let var = crate::hir_def::pous::variable::VariableDecl::new(
            self.db, name, name_id, kind, variadic, spec, init, id, scope_id,
        );
        self.register_node(id, HirNode::VariableDecl(var));
        var
    }

    /// Create an Expr, register it in the node index, and return it.
    pub fn new_expr(
        &mut self,
        kind: crate::hir_def::expressions::expression::ExprKind<'db>,
        id: AstId,
        scope_id: ScopeId<'db>,
    ) -> crate::hir_def::expressions::expression::Expr<'db> {
        let expr = crate::hir_def::expressions::expression::Expr::new(self.db, kind, id, scope_id);
        self.register_node(id, HirNode::Expr(expr));
        expr
    }

    /// Create a Spec, register it in the node index, and return it.
    pub fn new_spec(
        &mut self,
        kind: crate::hir_def::expressions::spec::SpecKind<'db>,
        id: AstId,
        scope_id: ScopeId<'db>,
    ) -> crate::hir_def::expressions::spec::Spec<'db> {
        let spec = crate::hir_def::expressions::spec::Spec::new(self.db, kind, id, scope_id);
        self.register_node(id, HirNode::Spec(spec));
        spec
    }

    /// Create a VariableAccess, register it in the node index, and return it.
    pub fn new_variable_access(
        &mut self,
        kind: crate::hir_def::expressions::expression::VariableAccessKind<'db>,
        multibits: Option<crate::hir_def::expressions::expression::MultibitsPart>,
        id: AstId,
        scope_id: ScopeId<'db>,
    ) -> crate::hir_def::expressions::expression::VariableAccess<'db> {
        let access = crate::hir_def::expressions::expression::VariableAccess::new(
            self.db, kind, multibits, id, scope_id,
        );
        self.register_node(id, HirNode::VariableAccess(access));
        access
    }

    /// Create an Invocation, register it in the node index, and return it.
    pub fn new_invocation(
        &mut self,
        id: AstId,
        name_id: AstId,
        scope_id: ScopeId<'db>,
        kind: crate::hir_def::expressions::invocation::InvocationKind,
    ) -> crate::hir_def::expressions::invocation::Invocation<'db> {
        let inv = crate::hir_def::expressions::invocation::Invocation::new(
            self.db, id, name_id, scope_id, kind,
        );
        self.register_node(id, HirNode::Invocation(inv));
        inv
    }

    /// Create a ParamAssign, register it in the node index, and return it.
    pub fn new_param(
        &mut self,
        id: AstId,
        scope_id: ScopeId<'db>,
        kind: crate::hir_def::expressions::expression::ParamAssignKind<'db>,
    ) -> crate::hir_def::expressions::expression::ParamAssign<'db> {
        let param =
            crate::hir_def::expressions::expression::ParamAssign::new(self.db, id, scope_id, kind);
        self.register_node(id, HirNode::Param(param));
        param
    }

    /// Create a StructElement, register it in the node index, and return it.
    pub fn new_struct_element(
        &mut self,
        name: crate::hir_def::interned::identifier::Ident,
        name_id: AstId,
        located: Option<crate::hir_def::expressions::expression::VariableAccess<'db>>,
        multibits: Option<crate::hir_def::expressions::expression::MultibitsPart>,
        spec: crate::hir_def::expressions::spec::Spec<'db>,
        init: Option<crate::hir_def::expressions::expression::InitExpr<'db>>,
        id: AstId,
        scope_id: ScopeId<'db>,
    ) -> crate::hir_def::expressions::spec::StructElement<'db> {
        let elem = crate::hir_def::expressions::spec::StructElement::new(
            self.db, name, name_id, located, multibits, spec, init, id, scope_id,
        );
        self.register_node(id, HirNode::StructElement(elem));
        elem
    }

    /// Create a PathExpr, register it in the node index, and return it.
    pub fn new_path_expr(
        &mut self,
        kind: crate::hir_def::expressions::expression::PathExprKind<'db>,
        id: AstId,
        scope_id: ScopeId<'db>,
    ) -> crate::hir_def::expressions::expression::PathExpr<'db> {
        let path =
            crate::hir_def::expressions::expression::PathExpr::new(self.db, kind, id, scope_id);
        self.register_node(id, HirNode::PathExpr(path));
        path
    }

    /// Create an InitExpr, register it in the node index, and return it.
    pub fn new_init_expr(
        &mut self,
        kind: crate::hir_def::expressions::expression::InitExprKind<'db>,
        id: AstId,
        scope_id: ScopeId<'db>,
    ) -> crate::hir_def::expressions::expression::InitExpr<'db> {
        let init =
            crate::hir_def::expressions::expression::InitExpr::new(self.db, kind, id, scope_id);
        self.register_node(id, HirNode::InitExpr(init));
        init
    }

    /// Register a scope in the scope map.
    pub fn register_scope(
        &mut self,
        kind: ScopeKind<'db>,
        usings: Vec<Using<'db>>,
        scope_id: ScopeId<'db>,
        visibility: Visibility,
        parent: ScopeId<'db>,
    ) {
        let scope = Scope::new(self.file, kind, usings, scope_id, visibility, Some(parent));
        self.scope_keys
            .insert(scope_id.scope(self.db), Arc::new(scope));
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<SpanIdent<'db>>, IdeDiagnostic> {
        namespace
            .name
            .cast(self.ast)
            .children
            .iter()
            .map(|n| SpanIdent::new(self.db, self, n))
            .collect::<Result<Vec<_>, IdeDiagnostic>>()
    }

    // Fix me: This function should not panic, but handle errors gracefully.
    #[tracing::instrument(skip_all, name = "build HIR")]
    pub fn build(mut self) -> SemanticIndex<'db> {
        let mut usings = vec![];
        let global_scope = ScopeId::global(self.db, self.file);

        for child in self.source.children.iter() {
            type SourceFileDecl = ast::generated::ERRInvalidPouKeyword_ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective;

            self.current_scope = global_scope;
            match child.cast(self.ast) {
                SourceFileDecl::ERRInvalidPouKeyword(err) => self
                    .errors
                    .push(SyntaxError::InvalidPouKeyword(err.get_span()).to_diagnostic(self.db)),
                SourceFileDecl::NamespaceDecl(namespace) => {
                    let path = match self.get_namespace_path(namespace) {
                        Ok(path) => path,
                        Err(err) => {
                            self.errors.push(err);
                            continue;
                        }
                    };

                    match self.parse_namespace(&path, namespace) {
                        Ok(ns) => {
                            self.namespaces.push(ns);
                        }
                        Err(err) => {
                            self.errors.push(err);
                            continue;
                        }
                    }
                }
                SourceFileDecl::UsingDirective(directive) => match self.parse_using(directive) {
                    Ok(u) => usings.extend(u),
                    Err(err) => self.errors.push(err),
                },
                SourceFileDecl::FuncDecl(func) => match self.parse_function(func) {
                    Ok(r) => self.global_pous.push(r),
                    Err(err) => self.errors.push(err),
                },
                SourceFileDecl::FbDecl(fb) => match self.parse_function_block(fb) {
                    Ok(r) => self.global_pous.push(r),
                    Err(err) => self.errors.push(err),
                },
                SourceFileDecl::ClassDecl(class) => match self.parse_class(class) {
                    Ok(r) => self.global_pous.push(r),
                    Err(err) => self.errors.push(err),
                },
                SourceFileDecl::DataTypeDecl(data_type) => {
                    for child in &data_type.children {
                        match self.parse_data_type(child.cast(self.ast)) {
                            Ok(r) => self.global_pous.push(r),
                            Err(err) => self.errors.push(err),
                        }
                    }
                }
                SourceFileDecl::InterfaceDecl(interface) => match self.parse_interface(interface) {
                    Ok(r) => self.global_pous.push(r),
                    Err(err) => self.errors.push(err),
                },
                SourceFileDecl::ConfigDecl(config) => match self.parse_config(config) {
                    Ok(c) => {
                        self.register_node(c.span(self.db), HirNode::Config(c));
                        self.configs.push(c);
                    }
                    Err(err) => self.errors.push(err),
                },
                SourceFileDecl::ProgDecl(prog) => match self.parse_program(prog) {
                    Ok(p) => self.programs.push(p),
                    Err(err) => self.errors.push(err),
                },
            }
        }

        let scope = Scope::new(
            self.file,
            ScopeKind::Global,
            usings,
            self.current_scope,
            Visibility::PUBLIC,
            None,
        );

        self.scope_keys
            .insert(global_scope.scope(self.db), Arc::new(scope));

        self.node_index
            .raw
            .sort_unstable_by_key(|node| *node.get_id(self.db));

        SemanticIndex {
            scope: global_scope,
            file: self.file,
            ast: Arc::clone(&self.ast.nodes),
            scopes: self.scope_keys,
            node_index: self.node_index,
            programs: self.programs,
            configs: self.configs,
            global_namespaces: self.global_namespaces,
            namespaces: self.namespaces,
            global_pous: self.global_pous,
            errors: self.errors,
        }
    }
}
