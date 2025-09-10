use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::statement::ParseStatement;
use crate::builder::{ParseSpec, ParseVarSection};
use crate::check::errors::sem_errors::{AnalysisError, SyntaxError};
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::hir_def::modifier::Modifier;
use crate::hir_def::pous::class::{Class, MethodDecl};
use crate::hir_def::pous::pou::{Pou, PouDecl};
use crate::hir_def::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use ast::generated::{ClassDecl, ClassVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_class(
        &mut self,
        class: &ClassDecl,
    ) -> anyhow::Result<PouDecl<'db>, AnalysisError<'db>> {
        let scope_id = FileScopeId::from((self.db, self.file, class.get_id()));
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let extends = class.extends.as_ref().and_then(|e| {
            match SpanNamespaceAccess::from_ast(self.db, self, e.cast(self.ast)) {
                Ok(namespace) => Some(namespace),
                Err(error) => {
                    self.errors.push(error);
                    None
                }
            }
        });

        let implements = class
            .implements
            .as_ref()
            .map(|i| {
                i.cast(self.ast)
                    .children
                    .iter()
                    .filter_map(|i| {
                        match SpanNamespaceAccess::from_ast(self.db, self, i.cast(self.ast)) {
                            Ok(namespace) => Some(namespace),
                            Err(error) => {
                                self.errors.push(error);
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut modifiers = Modifier::empty();
        class.qualifier.as_ref().map(|q| match q.cast(self.ast) {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifier::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifier::FINAL),
        });

        let mut variables = vec![];

        for v in class.variables.iter() {
            match v.cast(self.ast) {
                ClassVariables::ExternalVarDecls(e) => e.parse(self, &mut variables),
                ClassVariables::LocPartlyVarDecl(i) => i.parse(self, &mut variables),
                ClassVariables::NoRetainVarDecls(i) => i.parse(self, &mut variables),
                ClassVariables::RetainVarDecls(i) => i.parse(self, &mut variables),
                ClassVariables::VarDecls(i) => i.parse(self, &mut variables),
            }
        }

        class.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.cast(self.ast) {
                Error::ERRExtendsMultipleTimes(err) => {
                    self.errors.push(AnalysisError::SyntaxError(SyntaxError::MultipleExtends(err.get_span())));
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    self.errors.push(AnalysisError::SyntaxError(SyntaxError::ImplementsBeforeExtends(err.get_span())));
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    self.errors.push(AnalysisError::SyntaxError(SyntaxError::MultipleImplements(err.get_span())));
                },
            }
        });

        let methods = class.methods
            .iter()
            .filter_map(|m| {
            let name = match Ident::from_node(self.db, self.file, m.cast(self.ast).name.cast(self.ast)) {
                Ok(name) => name,
                Err(error) => {
                self.errors.push(error);
                return None;
                }
            };

            let mut modifiers = match m.cast(self.ast).modifier.as_ref().map(|m| m.cast(self.ast)) {
                Some(ast::generated::Operators_2::Token_ABSTRACT(_)) => Modifier::ABSTRACT,
                Some(ast::generated::Operators_2::Token_FINAL(_)) => Modifier::FINAL,
                _ => Modifier::empty(),
            };

            if m.cast(&self.ast)._override.is_some() {
                modifiers |= Modifier::OVERRIDE;
            }

            type MethodBody = ast::generated::ExternalVarDecls_InOutDecls_InputDecls_OutputDecls_TempVarDecls_VarDecls;
            let mut method_variables = vec![];
            for v in m.cast(self.ast).variables.iter() {
                match v.cast(self.ast) {
                MethodBody::ExternalVarDecls(decls) => decls.parse(self, &mut method_variables),
                MethodBody::InOutDecls(decls) => decls.parse(self, &mut method_variables),
                MethodBody::InputDecls(decls) => decls.parse(self, &mut method_variables),
                MethodBody::OutputDecls(decls) => decls.parse(self, &mut method_variables),
                MethodBody::TempVarDecls(decls) => decls.parse(self, &mut method_variables),
                MethodBody::VarDecls(decls) => decls.parse(self, &mut method_variables),
                }
            }

            let mut body = vec![];
            if let Some(body_node) = m.cast(self.ast).body.as_ref() {
                if let ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(stmts) = body_node.cast(self.ast).children.cast(self.ast) {
                for stmt in stmts.children.iter() {
                    match stmt.cast(self.ast).to_statement(self) {
                    Ok(statement) => body.push(statement),
                    Err(error) => self.errors.push(error),
                    }
                }
                }
            }

            let return_type: Option<_> = m
                .cast(self.ast)
                .return_type
                .as_ref()
                .and_then(|rt| {
                match rt.cast(self.ast).to_spec(self) {
                    Ok(spec) => Some(spec),
                    Err(error) => {
                    self.errors.push(error);
                    None
                    }
                }
                });

            let _override = m.cast(self.ast)._override.is_some();

            Some(MethodDecl::new(
                self.db,
                method_variables,
                name,
                return_type,
                modifiers,
                _override,
                body,
                m.cast(self.ast).into(),
                m.cast(self.ast).name.cast(self.ast).into(),
                scope_id
            ))
        }).collect::<Vec<_>>();

        let name = Ident::from_node(self.db, self.file, class.name.cast(self.ast))?;
        let usings = match self.parse_usings(&class.directives) {
            Ok(usings) => usings,
            Err(error) => {
                self.errors.push(error);
                vec![]
            }
        };

        let result = PouDecl::new(
            self.db,
            Pou::Class(Class::new(
                self.db, extends, implements, variables, methods, modifiers, scope_id,
            )),
            name,
            class.into(),
            class.name.cast(self.ast).into(),
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
}
