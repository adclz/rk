use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::tracked::ParsedAst;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::check::errors::sem_errors::{AnalysisError, SyntaxError};
use crate::def::interned::identifier::SpannedIdent;
use crate::def::namespace::Namespace;
use crate::def::pous::pou::PouDecl;
use crate::def::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use crate::def::semantic_index::SemanticIndex;

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

    pub(crate) errors: Vec<AnalysisError<'db>>
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
            pous: vec![],
            namespaces: vec![],
            current_scope: FileScopeId::global(file),
            errors: vec![]
        }
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<SpannedIdent>, AnalysisError<'db>> {
        namespace
            .name
            .cast(&self.ast)
            .children
            .iter()
            .map(|n| SpannedIdent::new(self.db, self, &n))
            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
    }

    pub fn create_pou_id(&self, node: &impl AstNode) -> FileScopeId {
        
        FileScopeId::from((self.file, node.get_id()))
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
                    self.errors.push(AnalysisError::SyntaxError(SyntaxError::InvalidPouKeyword(err.get_span())))
                }
                SourceFileDecl::NamespaceDecl(namespace) => {
                    let path = match self.get_namespace_path(namespace) {
                        Ok(path) => path,
                        Err(err) => {
                            self.errors.push(err);
                            continue;
                        }
                    };
                    let namespace_id = namespace.get_id();

                    if let Err(err) = self.parse_namespace(&path, namespace) {
                        self.errors.push(err);
                        continue;
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
            ast: self.ast.nodes.clone(),
            scopes: self.scope_keys,
            namespaces: self.namespaces,
            global_pous: self.pous,
            errors: self.errors,
        }
    }
}
