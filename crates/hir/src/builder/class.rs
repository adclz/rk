use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{Parse, ParseSpec, ParseVarSection};
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::class::{Class, MethodDecl};
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::{ScopeId, ScopeKind};
use crate::hir_ty::head::inheritance::MethodRef;
use crate::{Modifier, Visibility};
use ast::generated::{ClassDecl, ClassVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::{AstNode, AstNodeId};
use ide_diagnostic::IdeDiagnostic;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_class(&mut self, class: &ClassDecl) -> anyhow::Result<Pou<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let extends = class.extends.as_ref().and_then(|e| {
            let spec = e.cast(self.ast).to_spec(self);
            self.try_parse(spec)
        });

        let implements = class
            .implements
            .as_ref()
            .map(|i| {
                i.cast(self.ast)
                    .children
                    .iter()
                    .filter_map(|i| {
                        let spec = i.cast(self.ast).to_spec(self);
                        self.try_parse(spec)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut modifiers = Modifier::empty();
        if let Some(class_mod) = &class.modifier {
            match class_mod.cast(self.ast) {
                ast::generated::Operators_2::Token_ABSTRACT(_) => {
                    modifiers.insert(Modifier::ABSTRACT)
                }
                ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifier::FINAL),
            }
        }

        let mut variables = vec![];
        for v in class.variables.iter() {
            match v.cast(self.ast) {
                ClassVariables::ERRVarInOutNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarInOutNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ERRVarTempNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarTempNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ERRVarAccessNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarAccessNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ERRVarConfigNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarConfigNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ERRVarLocatedNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarLocatedNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ERRVarExternalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarExternalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ERRVarGlobalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarGlobalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ClassVariables::ExternalVarDecls(e) => e.parse(self, &mut variables),
                ClassVariables::LocPartlyVarDecl(i) => i.parse(self, &mut variables),
                ClassVariables::NoRetainVarDecls(i) => i.parse(self, &mut variables),
                ClassVariables::RetainVarDecls(i) => i.parse(self, &mut variables),
                ClassVariables::VarDecls(i) => i.parse(self, &mut variables),
            }
        }

        class.children.iter().for_each(|f| {
            type Error = ast::generated::ERRClassVariablesAfterMethod_ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.cast(self.ast) {
                Error::ERRExtendsMultipleTimes(err) => {
                    self.errors.push(SyntaxError::MultipleExtends {
                        location: err.get_range().to_owned(),
                        first_extend_span: class.extends.as_ref().unwrap().cast(self.ast).get_range().to_owned(),
                        file: self.file,
                    }.to_diagnostic(self.db, self.file));
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    self.errors.push(SyntaxError::ImplementsBeforeExtends {
                        implements_span: err.get_range().to_owned(),
                        extends_span: class.extends.as_ref().unwrap().cast(self.ast).get_range().to_owned(),
                        file: self.file,
                    }.to_diagnostic(self.db, self.file));
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    self.errors.push(SyntaxError::MultipleImplements {
                        location: err.get_range().to_owned(),
                        first_implements_span: class.implements.as_ref().unwrap().cast(self.ast).get_range().to_owned(),
                        file: self.file,
                    }.to_diagnostic(self.db, self.file));
                },
                Error::ERRClassVariablesAfterMethod(err) => {
                    let first_method_span = class.methods.first().unwrap().cast(self.ast).get_range().to_owned();
                    self.errors.push(SyntaxError::ClassVariablesAfterMethod {
                        var_span: err.get_range().to_owned(),
                        method_span: first_method_span,
                        file: self.file
                    }.to_diagnostic(self.db, self.file));
                },
            }
        });

        let name = Ident::from_node(self.db, self.file, class.name.cast(self.ast))?;
        let usings = self.parse_usings(&class.directives);
        let usings = self.parse_or_default(usings);

        let result = Pou::Class(Class::new(
            self.db,
            name,
            class.name.cast(self.ast).into(),
            extends,
            implements,
            variables,
            self.parse_methods(&class.methods),
            modifiers,
            class.into(),
            scope_id,
        ));

        self.register_node(class.into(), HirNode::PouDecl(result));
        self.register_scope(
            ScopeKind::Pou(result),
            usings,
            scope_id,
            Visibility::empty(),
            previous_scope,
        );

        Ok(result)
    }

    pub fn parse_methods(
        &mut self,
        methods: &[AstNodeId<ast::generated::MethodDecl>],
    ) -> Vec<MethodDecl<'db>> {
        let previous_scope = self.current_scope;

        methods
            .iter()
            .filter_map(|m| self.parse_single_method(m, previous_scope))
            .collect()
    }

    fn parse_single_method(
        &mut self,
        m: &AstNodeId<ast::generated::MethodDecl>,
        parent_scope: ScopeId<'db>,
    ) -> Option<MethodDecl<'db>> {
        let scope_id = self.generate_scope_id();
        self.current_scope = scope_id;

        let method = m.cast(self.ast);

        let name = self.try_parse(Ident::from_node(
            self.db,
            self.file,
            method.name.cast(self.ast),
        ))?;

        let mut modifiers = match method.modifier.as_ref().map(|m| m.cast(self.ast)) {
            Some(ast::generated::Operators_2::Token_ABSTRACT(_)) => Modifier::ABSTRACT,
            Some(ast::generated::Operators_2::Token_FINAL(_)) => Modifier::FINAL,
            _ => Modifier::empty(),
        };

        if method._override.is_some() {
            modifiers |= Modifier::OVERRIDE;
        }

        let mut variables = vec![];
        for v in method.variables.iter() {
            match v.cast(self.ast) {
                ast::generated::MethodDeclVariables::ERRVarAccessNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarAccessNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodDeclVariables::ERRVarConfigNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarConfigNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodDeclVariables::ERRVarExternalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarExternalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodDeclVariables::ERRVarGlobalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarGlobalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodDeclVariables::ERRVarLocatedNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarLocatedNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::MethodDeclVariables::ExternalVarDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodDeclVariables::InOutDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodDeclVariables::InputDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodDeclVariables::OutputDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodDeclVariables::TempVarDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
                ast::generated::MethodDeclVariables::VarDecls(decls) => {
                    decls.parse(self, &mut variables)
                }
            }
        }

        let mut body = vec![];
        if let Some(body_node) = method.body.as_ref()
            && let ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(stmts) =
                body_node.cast(self.ast).children.cast(self.ast)
        {
            for stmt in stmts.children.iter() {
                let r = stmt.cast(self.ast).parse(self);
                if let Some(s) = self.try_parse(r) {
                    body.push(s);
                }
            }
        }

        let return_type = method
            .return_type
            .as_ref()
            .map(|rt| rt.cast(self.ast).to_spec(self));
        let return_type = return_type.and_then(|rt| self.try_parse(rt));

        let visibility = match &method.access {
            Some(access) => match access.cast(self.ast).children.cast(self.ast) {
                ast::generated::Internal_Private_Protected_Public::Private(_) => {
                    Visibility::PRIVATE
                }
                ast::generated::Internal_Private_Protected_Public::Protected(_) => {
                    Visibility::PROTECTED
                }
                ast::generated::Internal_Private_Protected_Public::Public(_) => Visibility::PUBLIC,
                ast::generated::Internal_Private_Protected_Public::Internal(_) => {
                    Visibility::INTERNAL
                }
            },
            None => Visibility::PROTECTED,
        };

        let pragmas = self.parse_pou_pragmas(&method.pragmas);

        let result = MethodDecl::new(
            self.db,
            name,
            method.name.cast(self.ast).into(),
            variables,
            return_type,
            modifiers,
            visibility,
            method._override.is_some(),
            body,
            pragmas,
            method.into(),
            scope_id,
        );

        self.register_node(
            method.into(),
            HirNode::MethodRef(MethodRef::Declared(result)),
        );
        self.register_scope(
            ScopeKind::MethodDecl(result),
            vec![],
            scope_id,
            result.visibility(self.db),
            parent_scope,
        );

        Some(result)
    }
}
