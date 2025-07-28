use std::ops::Deref;
use std::panic;

use ast::generated::ERRInvalidPouKeyword;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;
use salsa::Accumulator;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir::interned::identifier::{Ident, SpannedIdent};
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scopes::scope::{
    NamespaceId, PouId, Scope, ScopeId, ScopeKind, ScopedNamespaceId, ScopedPouId, Visibility,
};
use crate::hir::semantic_index::SemanticIndex;

pub struct SemanticIndexBuilder<'db> {
    source: &'db ast::generated::SourceFile,

    pub(crate) db: &'db dyn BaseDatabase,
    pub(crate) file: File,

    /// Maps scope IDs to their corresponding scopes.
    pub(crate) scope_keys: FxHashMap<ScopeId, Scope<'db>>,

    /// Maps namespace keys to their corresponding namespaces.
    pub(crate) namespace_keys: FxHashMap<NamespaceId, Namespace<'db>>,

    /// Maps POU keys to their corresponding POU declarations.
    pub(crate) pou_keys: FxHashMap<PouId, PouDecl<'db>>,

    /// Map of scope IDs to their containing namespaces
    pub(crate) scope_to_namespaces: FxHashMap<ScopeId, FxHashMap<NamespacePath, ScopedNamespaceId>>,

    /// Map of scope IDs to their containing POUs
    pub(crate) scope_to_pous: FxHashMap<ScopeId, FxHashMap<Ident, ScopedPouId>>,

    /// The current scope ID being processed (by default, the global scope).
    pub(crate) current_scope: ScopeId,
}

impl<'db> SemanticIndexBuilder<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        file: File,
        source: &'db ast::generated::SourceFile,
    ) -> Self {
        Self {
            db,
            file,
            source,
            namespace_keys: FxHashMap::default(),
            pou_keys: FxHashMap::default(),
            scope_keys: FxHashMap::default(),
            scope_to_namespaces: FxHashMap::default(),
            scope_to_pous: FxHashMap::default(),
            current_scope: ScopeId::global(),
        }
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<SpannedIdent>> {
        namespace
            .name
            .children
            .iter()
            .map(|n| SpannedIdent::new(self.db, self.file, n.deref()))
            .collect::<anyhow::Result<Vec<_>>>()
    }

    pub fn insert_pou(&mut self, pou_id: PouId, pou_name: Ident, pou_decl: PouDecl<'db>) {
        self.pou_keys.insert(pou_id, pou_decl);
        self.scope_to_pous
            .entry(self.current_scope)
            .or_default()
            .insert(pou_name, ScopedPouId(pou_id, self.file));
    }

    pub fn create_pou_error(&self, err: &ERRInvalidPouKeyword) {
        let diag = diag()
            .message("Expected a POU keyword".into())
            .range(err.get_span())
            .call();
        DiagnosticAccumulator::accumulate(diag.into(), self.db);
    }

    // Fix me: This function should not panic, but handle errors gracefully.
    pub fn build(mut self) -> SemanticIndex<'db> {
        let mut usings = vec![];

        for child in self.source.children.iter() {
            type SourceFileDecl = ast::generated::ERRInvalidPouKeyword_ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective;

            match child.as_ref() {
                SourceFileDecl::ERRInvalidPouKeyword(err) => {
                    self.create_pou_error(err);
                }
                SourceFileDecl::NamespaceDecl(namespace) => {
                    let path = match self.get_namespace_path(namespace) {
                        Ok(path) => path,
                        Err(_err) => {
                            panic!("Failed to build namespace: {:?}", _err);
                        }
                    };
                    let namespace_id = namespace.get_id();

                    if let Err(e) = self.parse_namespace(&path, namespace) {
                        panic!("Failed to build namespace: {:?}", e);
                    }
                }
                SourceFileDecl::UsingDirective(directive) => {
                    let r = self.parse_using(directive).unwrap();
                    usings.extend(r);
                }
                SourceFileDecl::FuncDecl(func) => {
                    self.parse_function(func).unwrap();
                }
                SourceFileDecl::FbDecl(fb) => {
                    self.parse_function_block(fb).unwrap();
                }
                SourceFileDecl::ClassDecl(class) => {
                    self.parse_class(class).unwrap();
                }
                SourceFileDecl::DataTypeDecl(data_type) => {
                    self.parse_data_type(data_type).unwrap();
                }
                SourceFileDecl::InterfaceDecl(interface) => {
                    self.parse_interface(interface).unwrap();
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

        self.scope_keys.insert(ScopeId::global(), scope);

        debug_assert!(!self
            .namespace_keys
            .keys()
            .any(|k| self.pou_keys.contains_key(&PouId::from(k.0))));

        SemanticIndex {
            file: self.file,
            namespace_keys: self.namespace_keys,
            pou_keys: self.pou_keys,
            scopes: self.scope_keys,
            scope_to_namespaces: self.scope_to_namespaces,
            scope_to_pous: self.scope_to_pous,
        }
    }
}
