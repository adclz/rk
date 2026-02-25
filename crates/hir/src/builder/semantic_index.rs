use std::sync::Arc;

use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::ParsedAst;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::Visibility;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::config::ConfigDecl;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::program::ProgramDecl;
use crate::hir_def::scope::{Scope, ScopeId, ScopeKind};
use crate::hir_def::semantic_index::SemanticIndex;

pub struct SemanticIndexBuilder<'db> {
    pub(crate) source: &'db ast::generated::SourceFile,
    pub(crate) ast: &'db ParsedAst,

    pub(crate) db: &'db dyn WorkspaceDataBase,
    pub(crate) file: File,

    /// The current scope ID being processed (by default, the global scope).
    pub(crate) current_scope: ScopeId<'db>,

    /// Maps scope IDs to their corresponding scopes.
    pub(crate) scope_keys: FxHashMap<usize, Arc<Scope<'db>>>,

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
                SourceFileDecl::ERRInvalidPouKeyword(err) => {
                    self.errors
                        .push(SyntaxError::InvalidPouKeyword(
                            err.get_span(),
                        ).to_diagnostic(self.db))
                }
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
                SourceFileDecl::UsingDirective(directive) => {
                    match self.parse_using(directive) {
                        Ok(u) => usings.extend(u),
                        Err(err) => self.errors.push(err),
                    }
                }
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
                SourceFileDecl::InterfaceDecl(interface) => {
                    match self.parse_interface(interface) {
                        Ok(r) => self.global_pous.push(r),
                        Err(err) => self.errors.push(err),
                    }
                }
                SourceFileDecl::ConfigDecl(config) => match self.parse_config(config) {
                    Ok(c) => self.configs.push(c),
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

        SemanticIndex {
            scope: global_scope,
            file: self.file,
            ast: Arc::clone(&self.ast.nodes),
            scopes: self.scope_keys,
            programs: self.programs,
            configs: self.configs,
            global_namespaces: self.global_namespaces,
            namespaces: self.namespaces,
            global_pous: self.global_pous,
            errors: self.errors,
        }
    }
}
