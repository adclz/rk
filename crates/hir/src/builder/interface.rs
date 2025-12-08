use std::sync::Arc;

use crate::{Modifier, Visibility};
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{ParseSpec, ParseVarSection};
use crate::check::errors::analysis_error::AnalysisError;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::hir_def::pous::interface::{Interface, MethodPrototype};
use crate::hir_def::pous::pou::{Pou};
use crate::hir_def::scope::{Scope, ScopeKind};
use auto_lsp::anyhow;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_interface(
        &mut self,
        interface: &ast::generated::InterfaceDecl,
    ) -> anyhow::Result<Pou<'db>, AnalysisError<'db>> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, interface.name.cast(self.ast))?;
        let extends = interface
            .extends
            .as_ref()
            .map(|i| {
                i.cast(self.ast)
                    .children
                    .iter()
                    .filter_map(|i| {
                        match SpanNamespaceAccess::from_ast(self.db, self, i.cast(self.ast)) {
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
            .filter_map(|m| match self.parse_method_prototype(m.cast(self.ast)) {
                Ok(method) => Some(method),
                Err(error) => {
                    self.errors.push(error);
                    None
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

        let result = 
            Pou::Interface(Interface::new(
                self.db,
                name,
                interface.name.cast(self.ast).into(),
                extends,
                methods,
                interface.into(),
                scope_id,
            ));

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(result),
            usings,
            scope_id,
            Visibility::empty(),
            Some(previous_scope),
        );

        self.scope_keys
            .insert(scope_id.scope(self.db), Arc::new(scope));

        Ok(result)
    }

    pub fn parse_method_prototype(
        &mut self,
        method: &ast::generated::MethodPrototype,
    ) -> anyhow::Result<MethodPrototype<'db>, AnalysisError<'db>> {
        let name = Ident::from_node(self.db, self.file, method.name.cast(self.ast))?;
        let return_type = method
            .data_type
            .as_ref()
            .map(|i| match i.cast(self.ast) {
                ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                    elem_type_name.to_spec(self)
                }
                ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(self),
            })
            .transpose()?;

        let mut variables = vec![];
        for variable in method.variables.iter() {
            match variable.cast(self.ast) {
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
            method.name.cast(self.ast).into(),
            return_type,
            variables,
            method.into(),
            self.current_scope,
        ))
    }
}
