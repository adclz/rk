use crate::check::errors::semantic_errors::{
    implements_before_extends, multiple_extends, multiple_implements,
};
use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::class::{Class, MethodDecl};
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::scope::{FileScopeId, Scope, ScopeKind, Visibility};
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
            .map(|e| SpannedNamespaceAccess::from_ast(self.db, self, e.cast(&self.ast)))
            .transpose()?;

        let implements = class
            .implements
            .as_ref()
            .map(|i| {
                i.cast(&self.ast).children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::from_ast(self.db, self, i.cast(&self.ast)))
                    .collect()
            })
            .transpose()?;

        let mut modifiers = Modifiers::empty();
        class.qualifier.as_ref().map(|q| match q.cast(&self.ast) {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifiers::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifiers::FINAL),
        });

        let mut variables = vec![];

        for v in class.variables.iter() {
            match v.cast(&self.ast) {
                ClassVariables::ExternalVarDecls(e) => e.parse(self, &mut variables)?,
                ClassVariables::LocPartlyVarDecl(i) => i.parse(self, &mut variables)?,
                ClassVariables::NoRetainVarDecls(i) => i.parse(self, &mut variables)?,
                ClassVariables::RetainVarDecls(i) => i.parse(self, &mut variables)?,
                ClassVariables::VarDecls(i) => i.parse(self, &mut variables)?,
            }
        }

        class.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.cast(&self.ast) {
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
                let name = Ident::from_node(self.db, self.file, &*m.cast(&self.ast).name.cast(&self.ast))?;
                let modifiers = match m.cast(self.ast).modifier.as_ref().map(|m| m.cast(&self.ast)) {
                    Some(ast::generated::Operators_2::Token_ABSTRACT(_)) => Modifiers::ABSTRACT,
                    Some(ast::generated::Operators_2::Token_FINAL(_)) => Modifiers::FINAL,
                    _ => Modifiers::empty(),
                };

                type MethodBody = ast::generated::ExternalVarDecls_InOutDecls_InputDecls_OutputDecls_TempVarDecls_VarDecls;
                let mut method_variables = vec![];
                for v in m.cast(&self.ast).variables.iter() {
                    match v.cast(&self.ast) {
                        MethodBody::ExternalVarDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::InOutDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::InputDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::OutputDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::TempVarDecls(decls) => decls.parse(self, &mut method_variables)?,
                        MethodBody::VarDecls(decls) => decls.parse(self, &mut method_variables)?,
                    }
                }

            let body = m.cast(&self.ast).body
            .as_ref()
            .map_or(vec![], |body| match body.cast(&self.ast).children.cast(&self.ast) {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(ref stmts) => stmts
                    .children
                    .iter()
                    .map(|stmt| {
                        stmt.cast(&self.ast).to_statement(self)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()
                    .unwrap_or_default(),
                _ => vec![],
            });   

                 let return_type: Option<_> = m
                    .cast(&self.ast)
                    .return_type
                    .as_ref()
                    .map(|rt| {
                        rt.cast(&self.ast).to_spec(self)
                    })
                    .transpose()?;   

                let _override = m.cast(&self.ast)._override.is_some();

                Ok(MethodDecl::new(
                    self.db,
                    name,
                    method_variables,
                    return_type,
                    modifiers,
                    _override,
                    body,
                    m.cast(&self.ast).into(),
                    scope_id
                )) 
        }).collect::<anyhow::Result<Vec<_>>>()?;

        let name = Ident::from_node(self.db, self.file, class.name.cast(&self.ast))?;
        let usings = self.parse_usings(&class.directives)?;

        let result = PouDecl::new(
            self.db,
            Pou::Class(Class::new(
                self.db, extends, implements, variables, methods, modifiers, scope_id,
            )),
            name,
            class.into(),
            class.name.cast(&self.ast).into(),
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
