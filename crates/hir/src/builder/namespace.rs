use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use super::semantic_index::SemanticIndexBuilder;
use crate::Visibility;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespacePath;
use crate::hir_def::namespace::NamespaceDecl;
use crate::hir_def::scope::ScopeKind;
use ide_diagnostic::IdeDiagnostic;
 
impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_namespace(
        &mut self,
        parent_path: &[SpanIdent],
        nested: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<NamespaceDecl<'db>, IdeDiagnostic> {
        type Decl =
            ast::generated::ERRConfigNotAllowedInNamespace_ERRInvalidPouKeyword_ERRProgramNotAllowedInNamespace_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

        let scope_id = self.generate_scope_id();
        let path = SpanNamespacePath::from((self.db, parent_path, self.current_scope));
        let usings = self.parse_usings(&nested.directives);
        let usings = self.parse_or_default(usings);

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
                        self.errors.push(
                            SyntaxError::InvalidPouKeyword(err.get_span()).to_diagnostic(self.db),
                        );
                    }
                    Decl::ERRProgramNotAllowedInNamespace(err) => {
                        self.errors.push(
                            SyntaxError::ProgramNotAllowedInNamespace(err.get_span())
                                .to_diagnostic(self.db),
                        );
                    }
                    Decl::ERRConfigNotAllowedInNamespace(err) => {
                        self.errors.push(
                            SyntaxError::ConfigNotAllowedInNamespace(err.get_span())
                                .to_diagnostic(self.db),
                        );
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

        self.register_node(nested.into(), HirNode::Namespace(result));
        self.register_scope(
            ScopeKind::Namespace(result),
            usings,
            scope_id,
            match nested.internal {
                Some(_) => Visibility::INTERNAL,
                None => Visibility::PUBLIC,
            },
            previous_scope,
        );

        // Then insert it into the map with its ID
        self.global_namespaces.push(result);

        Ok(result)
    }
}
