use ast::generated::ERRInvalidPouKeyword_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use super::semantic_index::SemanticIndexBuilder;
use crate::check::errors::analysis_error::AnalysisError;
use crate::check::errors::syntax::SyntaxError;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::{NamespacePath, SpanNamespacePath};
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::scope::{Scope, ScopeKind};
use crate::hir_def::visibility::Visibility;

impl<'db> SemanticIndexBuilder<'db> {
    #[must_use]
    pub fn parse_namespace(
        &mut self,
        parent_path: &[SpanIdent],
        nested: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<NamespaceDecl<'db>, AnalysisError<'db>> {
        type Decl =
            ERRInvalidPouKeyword_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

        let scope_id = self.generate_scope_id();
        let path = SpanNamespacePath::from((self.db, parent_path, self.current_scope));
        let usings = match self.parse_usings(&nested.directives) {
            Ok(usings) => usings,
            Err(error) => {
                self.errors.push(error);
                vec![]
            }
        };

        let mut namespaces = vec![];
        let mut pous = vec![];

        let previous_scope = self.current_scope;

        if let Some(elements) = &nested.elements {
            for child in elements.cast(self.ast).children.iter() {
                self.current_scope = scope_id;

                match child.cast(self.ast) {
                    Decl::NamespaceDecl(namespace) => {
                        let nested_path = {
                            let mut path = parent_path.to_vec();
                            path.extend(self.get_namespace_path(namespace)?);
                            path
                        };
                        let ns = self.parse_namespace(&nested_path, namespace)?;
                        namespaces.push(ns);
                    }
                    Decl::FuncDecl(func) => {
                        pous.push(self.parse_function(func)?);
                    }
                    Decl::FbDecl(fb) => {
                        pous.push(self.parse_function_block(fb)?);
                    }
                    Decl::ClassDecl(class) => {
                        pous.push(self.parse_class(class)?);
                    }
                    Decl::DataTypeDecl(data_type) => {
                        for data_type in &data_type.children {
                            pous.push(self.parse_data_type(data_type.cast(self.ast))?);
                        }
                    }
                    Decl::InterfaceDecl(interface) => {
                        pous.push(self.parse_interface(interface)?);
                    }
                    Decl::ERRInvalidPouKeyword(err) => {
                        self.errors.push(AnalysisError::SyntaxError(
                            SyntaxError::InvalidPouKeyword(err.get_span()),
                        ));
                    }
                }
            }
        }

        let result = NamespaceDecl::new(
            self.db,
            *path,
            pous.clone(),
            namespaces,
            nested.into(),
            nested.name.cast(self.ast).into(),
            scope_id,
        );

        let scope = Scope::new(
            self.file,
            ScopeKind::Namespace(result),
            usings,
            scope_id,
            match nested.internal {
                Some(_) => Visibility::INTERNAL,
                None => Visibility::PUBLIC,
            },
            Some(previous_scope),
        );

        self.scope_keys.insert(scope_id.scope(self.db), scope);

        // Then insert it into the map with its ID
        self.global_namespaces.push(result);

        Ok(result)
    }
}
