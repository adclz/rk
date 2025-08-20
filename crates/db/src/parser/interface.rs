use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::interface::{Interface, MethodPrototype};
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::{ParseSpec, ParseVarSection};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_interface(
        &mut self,
        interface: &ast::generated::InterfaceDecl,
    ) -> anyhow::Result<PouDecl<'db>> {
        let scope_id = FileScopeId::from((self.file, interface.get_id()));
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, interface.name.cast(&self.ast))?;
        let extends = interface
            .extends
            .as_ref()
            .map(|i| {
                i.cast(&self.ast).children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::from_ast(self.db, self, i.cast(&self.ast)))
                    .collect()
            })
            .transpose()?;

        let methods = interface
            .prototype
            .iter()
            .map(|m| self.parse_method_prototype(m.cast(&self.ast)))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let usings = self.parse_usings(&interface.directives)?;

        let result = PouDecl::new(
            self.db,
            Pou::Interface(Interface::new(self.db, extends, vec![], scope_id)),
            name,
            interface.into(),
            interface.name.cast(&self.ast).into(),
            scope_id,
        );

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(result),
            usings,
            scope_id,
            Visibility::empty(),
            Some(previous_scope),
        );

        self.scope_keys.insert(scope_id, scope);

        Ok(result)
    }

    pub fn parse_method_prototype<'a>(
        &'a self,
        method: &ast::generated::MethodPrototype,
    ) -> anyhow::Result<MethodPrototype<'a>> {
        let name = Ident::from_node(self.db, self.file, method.name.cast(&self.ast))?;
        let return_type = method
            .data_type
            .as_ref()
            .map(|i| match i.cast(&self.ast) {
                ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                    elem_type_name.to_spec(self)
                }
                ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(self),
            })
            .transpose()?;

        let mut variables = vec![];
        for variable in method.variables.iter() {
            match variable.cast(&self.ast) {
                ast::generated::InOutDecls_InputDecls_OutputDecls::InputDecls(decls) => {
                    decls.parse(self, &mut variables)?
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::InOutDecls(decls) => {
                    decls.parse(self, &mut variables)?
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::OutputDecls(decls) => {
                    decls.parse(self, &mut variables)?
                }
            }
        }

        Ok(MethodPrototype::new(
            self.db,
            name,
            return_type,
            variables,
            method.name.cast(&self.ast).into(),
            method.into(),
            self.current_scope
        ))
    }
}
