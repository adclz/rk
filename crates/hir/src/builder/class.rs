use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{Parse, ParseSpec, ParseVarSection};
use crate::check::errors::ToIdeDiagnostic;
use ide_diagnostic::IdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::hir_def::pous::class::{Class, MethodDecl};
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::{ScopeId, ScopeKind};
use crate::{Modifier, Visibility};
use ast::generated::{ClassDecl, ClassVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::{AstNode, AstNodeId};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_class(
        &mut self,
        class: &ClassDecl,
    ) -> anyhow::Result<Pou<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let extends = class
            .extends
            .as_ref()
            .and_then(|e| self.try_parse(SpanNamespaceAccess::from_ast(self.db, self, e.cast(self.ast))));

        let implements = class
            .implements
            .as_ref()
            .map(|i| {
                i.cast(self.ast)
                    .children
                    .iter()
                    .filter_map(|i| self.try_parse(SpanNamespaceAccess::from_ast(self.db, self, i.cast(self.ast))))
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
                    self.errors.push(SyntaxError::MultipleExtends(err.get_span()).to_diagnostic(self.db));
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    self.errors.push(SyntaxError::ImplementsBeforeExtends(err.get_span()).to_diagnostic(self.db));
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    self.errors.push(SyntaxError::MultipleImplements(err.get_span()).to_diagnostic(self.db));
                },
                Error::ERRClassVariablesAfterMethod(err) => {
                    self.errors.push(SyntaxError::ClassVariablesAfterMethod(err.get_span()).to_diagnostic(self.db));
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

        let name = self.try_parse(Ident::from_node(self.db, self.file, method.name.cast(self.ast)))?;

        let mut modifiers = match method.modifier.as_ref().map(|m| m.cast(self.ast)) {
            Some(ast::generated::Operators_2::Token_ABSTRACT(_)) => Modifier::ABSTRACT,
            Some(ast::generated::Operators_2::Token_FINAL(_)) => Modifier::FINAL,
            _ => Modifier::empty(),
        };

        if method._override.is_some() {
            modifiers |= Modifier::OVERRIDE;
        }

        type MethodBody = ast::generated::ExternalVarDecls_InOutDecls_InputDecls_OutputDecls_TempVarDecls_VarDecls;

        let mut variables = vec![];
        for v in method.variables.iter() {
            match v.cast(self.ast) {
                MethodBody::ExternalVarDecls(decls) => decls.parse(self, &mut variables),
                MethodBody::InOutDecls(decls) => decls.parse(self, &mut variables),
                MethodBody::InputDecls(decls) => decls.parse(self, &mut variables),
                MethodBody::OutputDecls(decls) => decls.parse(self, &mut variables),
                MethodBody::TempVarDecls(decls) => decls.parse(self, &mut variables),
                MethodBody::VarDecls(decls) => decls.parse(self, &mut variables),
            }
        }

        let mut body = vec![];
        if let Some(body_node) = method.body.as_ref()
            && let ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(stmts) = body_node.cast(self.ast).children.cast(self.ast)
        {
            for stmt in stmts.children.iter() {
                let r = stmt.cast(self.ast).parse(self);
                if let Some(s) = self.try_parse(r) {
                    body.push(s);
                }
            }
        }

        let return_type = method.return_type.as_ref().map(|rt| rt.cast(self.ast).to_spec(self));
        let return_type = return_type.and_then(|rt| self.try_parse(rt));

        let visibility = match &method.access {
            Some(access) => match access.cast(self.ast).children.cast(self.ast) {
                ast::generated::Internal_Private_Protected_Public::Private(_) => Visibility::PRIVATE,
                ast::generated::Internal_Private_Protected_Public::Protected(_) => Visibility::PROTECTED,
                ast::generated::Internal_Private_Protected_Public::Public(_) => Visibility::PUBLIC,
                ast::generated::Internal_Private_Protected_Public::Internal(_) => Visibility::INTERNAL,
            },
            None => Visibility::PROTECTED,
        };

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
            method.into(),
            scope_id,
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
