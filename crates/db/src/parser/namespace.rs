use ast::generated::ERRInvalidPouKeyword_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use super::semantic_index::SemanticIndexBuilder;
use crate::hir::interned::identifier::SpannedIdent;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::namespace::Namespace;
use crate::hir::scopes::scope::{
    NamespaceId, Scope, ScopeId, ScopeKind, ScopedNamespaceId, Visibility,
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_namespace(
        &mut self,
        parent_path: &[SpannedIdent],
        nested: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<()> {
        type Decl =
            ERRInvalidPouKeyword_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

        let scope_id = ScopeId::from(nested.get_id());
        let namespace_id = NamespaceId::from(nested.get_id());
        let path = NamespacePath::from((self.db, parent_path));
        let usings = self.parse_usings(&nested.directives)?;
        let mut pous = vec![];

        let scope = Scope::new(
            self.file,
            ScopeKind::Namespace(namespace_id),
            usings,
            scope_id,
            match nested.internal {
                Some(_) => Visibility::INTERNAL,
                None => Visibility::PUBLIC,
            },
            Some(self.current_scope),
        );

        self.scope_keys.insert(scope_id, scope);

        self.scope_to_namespaces
            .entry(scope_id)
            .or_default()
            .insert(path, ScopedNamespaceId(namespace_id, self.file));

        self.current_scope = scope_id;

        if let Some(elements) = nested.elements.as_ref() {
            for child in elements.children.iter() {
                match child.as_ref() {
                    Decl::NamespaceDecl(namespace) => {
                        let nested_path = {
                            let mut path = parent_path.to_vec();
                            path.extend(self.get_namespace_path(namespace)?);
                            path
                        };
                        self.parse_namespace(&nested_path, namespace)?;
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
                            pous.push(self.parse_data_type(data_type)?);
                        }
                    }
                    Decl::InterfaceDecl(interface) => {
                        pous.push(self.parse_interface(interface)?);
                    }
                    Decl::ERRInvalidPouKeyword(err) => {
                        self.create_pou_error(err);
                    }
                }
            }
        }

        let result = Namespace::new(
            self.db,
            nested.get_span(),
            path,
            nested.name.get_span(),
            pous,
            self.file,
            scope_id,
        );

        // Then insert it into the map with its ID
        self.namespace_keys
            .insert(namespace_id.into(), result.clone());

        Ok(())
    }
}
