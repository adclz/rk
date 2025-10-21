use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::tracked::ParsedAst;
use auto_lsp::default::db::{BaseDatabase, file::File};
use rustc_hash::FxHashMap;

use crate::check::errors::analysis_error::AnalysisError;
use crate::check::errors::syntax::SyntaxError;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::pous::pou::PouDecl;
use crate::hir_def::scope::{FileScopeId, Scope, ScopeKind};
use crate::hir_def::semantic_index::SemanticIndex;
use crate::hir_def::visibility::Visibility;

pub struct SemanticIndexBuilder<'db> {
    pub(crate) source: &'db ast::generated::SourceFile,
    pub(crate) ast: &'db ParsedAst,

    pub(crate) db: &'db dyn BaseDatabase,
    pub(crate) file: File,

    /// Maps scope IDs to their corresponding scopes.
    pub(crate) scope_keys: FxHashMap<FileScopeId<'db>, Scope<'db>>,

    pub(crate) global_namespaces: Vec<NamespaceDecl<'db>>,
    pub(crate) global_pous: Vec<PouDecl<'db>>,

    /// Maps scope IDs to their corresponding namespaces.
    pub(crate) namespaces: Vec<NamespaceDecl<'db>>,

    /// The current scope ID being processed (by default, the global scope).
    pub(crate) current_scope: FileScopeId<'db>,

    pub(crate) errors: Vec<AnalysisError<'db>>,
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
            global_namespaces: vec![],
            global_pous: vec![],
            namespaces: vec![],
            current_scope: FileScopeId::global(db, file),
            errors: vec![],
        }
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<SpanIdent<'db>>, AnalysisError<'db>> {
        namespace
            .name
            .cast(self.ast)
            .children
            .iter()
            .map(|n| SpanIdent::new(self.db, self, n))
            .collect::<Result<Vec<_>, AnalysisError<'db>>>()
    }

    // Fix me: This function should not panic, but handle errors gracefully.
    #[tracing::instrument(skip_all, name = "build HIR")]
    pub fn build(mut self) -> SemanticIndex<'db> {
        let mut usings = vec![];
        let global_scope = FileScopeId::global(self.db, self.file);

        for child in self.source.children.iter() {
            type SourceFileDecl = ast::generated::ERRInvalidPouKeyword_ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective;

            self.current_scope = global_scope;
            match child.cast(self.ast) {
                SourceFileDecl::ERRInvalidPouKeyword(err) => {
                    self.errors
                        .push(AnalysisError::SyntaxError(SyntaxError::InvalidPouKeyword(
                            err.get_span(),
                        )))
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
                    usings.extend(self.parse_using(directive).unwrap());
                }
                SourceFileDecl::FuncDecl(func) => {
                    let r = self.parse_function(func).unwrap();
                    self.global_pous.push(r);
                }
                SourceFileDecl::FbDecl(fb) => {
                    let r = self.parse_function_block(fb).unwrap();
                    self.global_pous.push(r);
                }
                SourceFileDecl::ClassDecl(class) => {
                    let r = self.parse_class(class).unwrap();
                    self.global_pous.push(r);
                }
                SourceFileDecl::DataTypeDecl(data_type) => {
                    for child in &data_type.children {
                        let r = self.parse_data_type(child.cast(self.ast)).unwrap();
                        self.global_pous.push(r);
                    }
                }
                SourceFileDecl::InterfaceDecl(interface) => {
                    let r = self.parse_interface(interface).unwrap();
                    self.global_pous.push(r);
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

        self.scope_keys.insert(global_scope, scope);

        SemanticIndex {
            file: self.file,
            ast: self.ast.nodes.clone(),
            scopes: self.scope_keys,
            global_namespaces: self.global_namespaces,
            namespaces: self.namespaces,
            global_pous: self.global_pous,
            errors: self.errors,
        }
    }
}
