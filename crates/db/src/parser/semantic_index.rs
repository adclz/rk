use std::panic;

use ast::generated::ERRInvalidPouKeyword;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::tracked::ParsedAst;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::check::errors::semantic_errors::invalid_pou_keyword;
use crate::hir::interned::identifier::SpannedIdent;
use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use crate::hir::semantic_index::SemanticIndex;

pub struct SemanticIndexBuilder<'db> {
    source: &'db ast::generated::SourceFile,
    pub ast: &'db ParsedAst,

    pub(crate) db: &'db dyn BaseDatabase,
    pub(crate) file: File,

    /// Maps scope IDs to their corresponding scopes.
    pub(crate) scope_keys: FxHashMap<FileScopeId, Scope<'db>>,

    /// Maps scope IDs to their corresponding namespaces.
    pub(crate) namespaces: Vec<Namespace<'db>>,

    pub(crate) pous: Vec<PouDecl<'db>>,

    /// The current scope ID being processed (by default, the global scope).
    pub(crate) current_scope: FileScopeId,
}

impl<'db> SemanticIndexBuilder<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
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
            pous: Vec::new(),
            namespaces: Vec::new(),
            current_scope: FileScopeId::global(file),
        }
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<SpannedIdent>> {
        namespace
            .name
            .cast(&self.ast)
            .children
            .iter()
            .map(|n| SpannedIdent::new(self.db, self.file, &n))
            .collect::<anyhow::Result<Vec<_>>>()
    }

    pub fn create_pou_id(&self, node: &impl AstNode) -> FileScopeId {
        
        FileScopeId::from((self.file, node.get_id()))
    }

    pub fn create_pou_error(&self, err: &ERRInvalidPouKeyword) {
        invalid_pou_keyword(self.db, err.get_span());
    }

    // Fix me: This function should not panic, but handle errors gracefully.
    #[tracing::instrument(skip_all, name = "build HIR")]
    pub fn build(mut self) -> SemanticIndex<'db> {
        let mut usings = vec![];

        for child in self.source.children.iter() {
            type SourceFileDecl = ast::generated::ERRInvalidPouKeyword_ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective;

            self.current_scope = FileScopeId::global(self.file);
            match child.cast(self.ast) {
                SourceFileDecl::ERRInvalidPouKeyword(err) => {
                    self.create_pou_error(err);
                }
                SourceFileDecl::NamespaceDecl(namespace) => {
                    let path = match self.get_namespace_path(namespace) {
                        Ok(path) => path,
                        Err(_err) => {
                            panic!("Failed to build namespace: {_err:?}");
                        }
                    };
                    let namespace_id = namespace.get_id();

                    if let Err(e) = self.parse_namespace(&path, namespace) {
                        panic!("Failed to build namespace: {e:?}");
                    }
                }
                SourceFileDecl::UsingDirective(directive) => {
                    usings.extend(self.parse_using(directive).unwrap());
                }
                SourceFileDecl::FuncDecl(func) => {
                    let r = self.parse_function(func).unwrap();
                    self.pous.push(r);
                }
                SourceFileDecl::FbDecl(fb) => {
                    let r = self.parse_function_block(fb).unwrap();
                    self.pous.push(r);
                }
                SourceFileDecl::ClassDecl(class) => {
                    let r = self.parse_class(class).unwrap();
                    self.pous.push(r);
                }
                SourceFileDecl::DataTypeDecl(data_type) => {
                    for child in &data_type.children {
                        let r = self.parse_data_type(child.cast(&self.ast)).unwrap();
                        self.pous.push(r);
                    }
                }
                SourceFileDecl::InterfaceDecl(interface) => {
                    let r = self.parse_interface(interface).unwrap();
                    self.pous.push(r);
                }
                _ => {
                    //todo: add config and program declarations
                }
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
            .insert(FileScopeId::global(self.file), scope);

        SemanticIndex {
            file: self.file,
            scopes: self.scope_keys,
            namespaces: self.namespaces,
            global_pous: self.pous,
        }
    }
}
