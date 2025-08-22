use crate::check::errors::sem_errors::AnalysisError;
use crate::def::interned::identifier::Ident;
use crate::def::interned::namespace::SpannedNamespaceAccess;
use crate::def::pous::interface::{Interface, MethodPrototype};
use crate::def::pous::pou::{Pou, PouDecl};
use crate::def::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{ParseSpec, ParseVarSection};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_interface(
        &mut self,
        interface: &ast::generated::InterfaceDecl,
    ) -> anyhow::Result<PouDecl<'db>, AnalysisError<'db>> {
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
                .filter_map(|i| {
                match SpannedNamespaceAccess::from_ast(self.db, self, i.cast(&self.ast)) {
                    Ok(namespace) => Some(Some(namespace)),
                    Err(error) => {
                    self.errors.push(error);
                    None
                    }
                }
                })
                .collect()
            })
            .unwrap_or_default();

        let methods = interface
            .prototype
            .iter()
            .filter_map(|m| {
            match self.parse_method_prototype(m.cast(&self.ast)) {
                Ok(method) => Some(method),
                Err(error) => {
                self.errors.push(error);
                None
                }
            }
            })
            .collect::<Vec<_>>();

        let usings = match self.parse_usings(&interface.directives) {
            Ok(usings) => usings,
            Err(error) => {
            self.errors.push(error);
            vec![] 
            }
        };

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

    pub fn parse_method_prototype(
        &mut self,
        method: &ast::generated::MethodPrototype,
    ) -> anyhow::Result<MethodPrototype<'db>, AnalysisError<'db>> {
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
                    decls.parse(self, &mut variables)
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::InOutDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::OutputDecls(decls) => {
                    decls.parse(self, &mut variables)
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
