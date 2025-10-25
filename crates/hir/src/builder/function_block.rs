use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::statement::ParseStatement;
use crate::builder::{ParseSpec, ParseVarSection};
use crate::check::errors::analysis_error::AnalysisError;
use crate::check::errors::syntax::SyntaxError;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::hir_def::modifier::Modifier;
use crate::hir_def::pous::class::MethodDecl;
use crate::hir_def::pous::function_block::FunctionBlock;
use crate::hir_def::pous::pou::{Pou, PouDecl};
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::scope::{ScopeId, Scope, ScopeKind};
use crate::hir_def::visibility::Visibility;
use ast::generated::{FbDecl, FbVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function_block(
        &mut self,
        func: &FbDecl,
    ) -> anyhow::Result<PouDecl<'db>, AnalysisError<'db>> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let variables = func.parse_variables(self);

        let extends = func.extends.as_ref().and_then(|e| {
            match SpanNamespaceAccess::from_ast(self.db, self, e.cast(self.ast)) {
                Ok(namespace) => Some(namespace),
                Err(error) => {
                    self.errors.push(error);
                    None
                }
            }
        });

        let implements = func
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

        func.children.iter().for_each(|f| {
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

        let statements = func.body.as_ref().map_or(vec![], |body| {
            match body.cast(self.ast).children.cast(self.ast) {
                ast::generated::SFC_FbDiagram_LadderDiagram_StmtList::StmtList(stmts) => stmts
                    .children
                    .iter()
                    .filter_map(|stmt| match stmt.cast(self.ast).to_statement(self) {
                        Ok(statement) => Some(statement),
                        Err(err) => {
                            self.errors.push(err);
                            None
                        }
                    })
                    .collect(),
                _ => vec![],
            }
        });

        let methods = func.method
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

            if m.cast(self.ast)._override.is_some() {
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
                match &m.cast(self.ast).access {
                    Some(access) => match access.cast(self.ast).children.cast(self.ast) {
                        ast::generated::Internal_Private_Protected_Public::Private(_) => Visibility::PRIVATE,
                        ast::generated::Internal_Private_Protected_Public::Protected(_) => Visibility::PROTECTED,
                        ast::generated::Internal_Private_Protected_Public::Public(_) => Visibility::PUBLIC,
                        ast::generated::Internal_Private_Protected_Public::Internal(_) => Visibility::INTERNAL,
                    },
                    None => Visibility::PROTECTED,
                },
                _override,
                body,
                m.cast(self.ast).into(),
                m.cast(self.ast).name.cast(self.ast).into(),
                scope_id
            ))
        }).collect::<Vec<_>>();

        let mut modifiers = Modifier::empty();
        if let Some(func_mod) = &func.qualifier {
            match func_mod.cast(self.ast) {
                ast::generated::Operators_2::Token_ABSTRACT(_) => {
                    modifiers.insert(Modifier::ABSTRACT)
                }
                ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifier::FINAL),
            }
        }

        let name = Ident::from_node(self.db, self.file, func.name.cast(self.ast))?;
        let usings = match self.parse_usings(&func.directives) {
            Ok(usings) => usings,
            Err(error) => {
                self.errors.push(error);
                vec![]
            }
        };
        let result = PouDecl::new(
            self.db,
            Pou::FunctionBlock(FunctionBlock::new(
                self.db, extends, implements, variables, methods, statements, modifiers, scope_id,
            )),
            name,
            func.into(),
            func.name.cast(self.ast).into(),
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

        self.scope_keys.insert(scope_id.scope(self.db), scope);

        Ok(result)
    }
}

trait ParseVariable<'db> {
    fn parse_variables(&self, sema: &mut SemanticIndexBuilder<'db>) -> Vec<VariableDecl<'db>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FbDecl {
    fn parse_variables(&self, sema: &mut SemanticIndexBuilder<'db>) -> Vec<VariableDecl<'db>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.cast(sema.ast) {
                FbVariables::FbInputDecls(decls) => decls.parse(sema, &mut variables),
                FbVariables::FbOutputDecls(decls) => decls.parse(sema, &mut variables),
                FbVariables::InOutDecls(decls) => decls.parse(sema, &mut variables),
                FbVariables::ExternalVarDecls(decls) => decls.parse(sema, &mut variables),
                FbVariables::TempVarDecls(decls) => decls.parse(sema, &mut variables),
                FbVariables::VarDecls(decls) => decls.parse(sema, &mut variables),
                FbVariables::LocPartlyVarDecl(loc_partly_var_decl) => {
                    loc_partly_var_decl.parse(sema, &mut variables)
                }
                FbVariables::NoRetainVarDecls(no_retain_var_decls) => {
                    no_retain_var_decls.parse(sema, &mut variables)
                }
                FbVariables::RetainVarDecls(retain_var_decls) => {
                    retain_var_decls.parse(sema, &mut variables)
                }
            }
        }

        variables
    }
}
