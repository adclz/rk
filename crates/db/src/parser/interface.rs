use std::ops::Deref;

use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::interface::{Interface, Method};
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::scopes::scope::{PouId, Scope, ScopeId, ScopeKind, Visibility};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::{ParseSpec, ParseVarSection};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_interface(
        &mut self,
        interface: &ast::generated::InterfaceDecl,
    ) -> anyhow::Result<PouId> {
        let id = ScopeId::from(interface.get_id());
        let pou_key = PouId::from(interface.get_id());
        let name = Ident::from_node(self.db, self.file, interface.name.deref())?;
        let extends = interface
            .extends
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::from_ast(self.db, self.file, i))
                    .collect()
            })
            .transpose()?;

        let methods = interface
            .prototype
            .iter()
            .map(|m| self.parse_method_prototype(m))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let usings = self.parse_usings(&interface.directives)?;

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(pou_key),
            usings,
            id,
            Visibility::empty(),
            Some(self.current_scope),
        );

        let result = Interface::new(self.db, extends, methods, self.current_scope);

        self.pou_keys.insert(
            pou_key,
            PouDecl::new(
                self.db,
                Pou::Interface(result),
                interface.get_span(),
                name,
                interface.name.get_span(),
            ),
        );

        Ok(pou_key)
    }

    pub fn parse_method_prototype(
        &mut self,
        method: &ast::generated::MethodPrototype,
    ) -> anyhow::Result<Method<'db>> {
        let name = Ident::from_node(self.db, self.file, &*method.name)?;
        let return_type = method
            .data_type
            .as_ref()
            .map(|i| match i.deref() {
                ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                    elem_type_name.to_spec(self.db, self.file)
                }
                ast::generated::DataTypeAccess::NamespaceAccess(target) => {
                    target.to_spec(self.db, self.file)
                }
            })
            .transpose()?;

        let mut variables = vec![];
        for variable in method.variables.iter() {
            match variable.deref() {
                ast::generated::InOutDecls_InputDecls_OutputDecls::InputDecls(decls) => {
                    decls.parse(self.db, self.file, &mut variables)?
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::InOutDecls(decls) => {
                    decls.parse(self.db, self.file, &mut variables)?
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::OutputDecls(decls) => {
                    decls.parse(self.db, self.file, &mut variables)?
                }
            }
        }

        Ok(Method::new(
            self.db,
            method.get_span(),
            name,
            method.name.get_span(),
            return_type,
            variables,
        ))
    }
}
