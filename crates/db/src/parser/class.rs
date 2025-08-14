use std::ops::Deref;

use crate::check::errors::semantic_errors::{
    implements_before_extends, multiple_extends, multiple_implements,
};
use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::class::{Class, MethodDecl};
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::scopes::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use crate::hir::visibility::Modifiers;
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::statement::ParseStatement;
use crate::parser::{ParseSpec, ParseVarSection};
use ast::generated::{ClassDecl, ClassVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_class(&mut self, class: &ClassDecl) -> anyhow::Result<PouDecl<'db>> {
        let scope_id = FileScopeId::from((self.file, class.get_id()));
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let extends = class
            .extends
            .as_ref()
            .map(|e| SpannedNamespaceAccess::from_ast(self.db, self.file, e))
            .transpose()?;

        let implements = class
            .implements
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::from_ast(self.db, self.file, i))
                    .collect()
            })
            .transpose()?;

        let mut modifiers = Modifiers::empty();
        class.qualifier.as_ref().map(|q| match q.deref() {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifiers::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifiers::FINAL),
        });

        let mut variables = vec![];

        for v in class.variables.iter() {
            match v.deref() {
                ClassVariables::ExternalVarDecls(e) => e.parse(self, &mut variables)?,
                ClassVariables::LocPartlyVarDecl(i) => i.parse(self, &mut variables)?,
                ClassVariables::NoRetainVarDecls(i) => i.parse(self, &mut variables)?,
                ClassVariables::RetainVarDecls(i) => i.parse(self, &mut variables)?,
                ClassVariables::VarDecls(i) => i.parse(self, &mut variables)?,
            }
        }

        class.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.deref() {
                Error::ERRExtendsMultipleTimes(err) => {
                    multiple_extends(self.db, err.get_span());
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    implements_before_extends(self.db, err.get_span());
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    multiple_implements(self.db, err.get_span());
                },
            }
        });

        let methods = class.methods
            .iter()
            .map(|m| {
                let name = Ident::from_node(self.db, self.file, &*m.name)?;
                let modifiers = match m.modifier.as_ref().map(|m| m.deref()) {
                    Some(ast::generated::Operators_2::Token_ABSTRACT(_)) => Modifiers::ABSTRACT,
                    Some(ast::generated::Operators_2::Token_FINAL(_)) => Modifiers::FINAL,
                    _ => Modifiers::empty(),
                };

                type MethodBody = ast::generated::ExternalVarDecls_InOutDecls_InputDecls_OutputDecls_TempVarDecls_VarDecls;
                let mut method_variables = vec![];
                for v in m.variables.iter() {
                    match v.deref() {
                        MethodBody::ExternalVarDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::InOutDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::InputDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::OutputDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::TempVarDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::VarDecls(decls) => decls.parse(self, &mut method_variables)?,
                    }
                }

            let body = m.body
            .as_ref()
            .map_or(vec![], |body| match body.children.deref() {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(ref stmts) => stmts
                    .children
                    .iter()
                    .map(|stmt| {
                        stmt.to_statement(self)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()
                    .unwrap_or_default(),
                _ => vec![],
            });   

                 let return_type: Option<_> = m
                    .return_type
                    .as_ref()
                    .map(|rt| {
                        rt.to_spec(self)
                    })
                    .transpose()?;   

                let _override = m._override.is_some();

                Ok(MethodDecl::new(
                    self.db,
                    name,
                    m.get_span(),
                    method_variables,
                    return_type,
                    modifiers,
                    _override,
                    scope_id,
                    body 
                )) 
        }).collect::<anyhow::Result<Vec<_>>>()?;

        let name = Ident::from_node(self.db, self.file, class.name.deref())?;
        let usings = self.parse_usings(&class.directives)?;

        let result = PouDecl::new(
            self.db,
            Pou::Class(Class::new(
                self.db, extends, implements, variables, methods, modifiers, scope_id,
            )),
            class.get_span(),
            name,
            class.name.get_span(),
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
