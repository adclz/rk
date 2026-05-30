use crate::Visibility;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{ParseSpec, ParseVarSection};
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::interface::{Interface, MethodPrototype};
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::{ScopeId, ScopeKind};
use crate::hir_ty::head::inheritance::MethodRef;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use ide_diagnostic::IdeDiagnostic;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_interface(
        &mut self,
        interface: &ast::generated::InterfaceDecl,
    ) -> anyhow::Result<Pou<'db>, IdeDiagnostic> {
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
                        let spec = i.cast(self.ast).to_spec(self);
                        self.try_parse(spec).map(Some)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let methods = interface
            .prototype
            .iter()
            .filter_map(|m| {
                let r = self.parse_method_prototype(m.cast(self.ast), previous_scope);
                self.try_parse(r)
            })
            .collect::<Vec<_>>();

        let usings = self.parse_usings(&interface.directives);
        let usings = self.parse_or_default(usings);

        let result = Pou::Interface(Interface::new(
            self.db,
            name,
            interface.name.cast(self.ast).into(),
            extends,
            methods,
            interface.into(),
            scope_id,
        ));

        self.register_node(interface.into(), HirNode::PouDecl(result));
        self.register_scope(
            ScopeKind::Pou(result),
            usings,
            scope_id,
            Visibility::empty(),
            previous_scope,
        );

        Ok(result)
    }

    pub fn parse_method_prototype(
        &mut self,
        method: &ast::generated::MethodPrototype,
        previous_scope: ScopeId<'db>,
    ) -> anyhow::Result<MethodPrototype<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, method.name.cast(self.ast))?;
        let return_type = method
            .data_type
            .as_ref()
            .map(|i| i.cast(self.ast).to_spec(self))
            .transpose()?;

        if let Some(err) = &method.children {
            self.errors.push(
                SyntaxError::AccessSpecNotAllowedInMethodPrototype(
                    err.cast(self.ast).get_range().to_owned(),
                )
                .to_diagnostic(self.db, self.file),
            );
        }

        let mut variables = vec![];
        for variable in method.variables.iter() {
            match variable.cast(self.ast) {
                ast::generated::MethodProtVariables::ERRVarAccessNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarAccessNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::ERRVarConfigNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarConfigNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::ERRVarLocatedNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarLocatedNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::ERRVarExternalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarExternalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::ERRVarGlobalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarGlobalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::ERRVarInOutNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarInOutNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::ERRVarTempNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarTempNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodProtVariables::InputDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodProtVariables::InOutDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodProtVariables::OutputDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
            }
        }

        let result = MethodPrototype::new(
            self.db,
            name,
            method.name.cast(self.ast).into(),
            return_type,
            variables,
            method.into(),
            scope_id,
        );

        self.register_node(
            method.into(),
            HirNode::MethodRef(MethodRef::Prototype(result)),
        );
        self.register_scope(
            ScopeKind::MethodProt(result),
            vec![],
            scope_id,
            Visibility::empty(),
            previous_scope,
        );

        Ok(result)
    }
}
