use std::ops::Deref;

use crate::hir::interned::identifier::Ident;
use crate::hir::pous::function::Function;
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::pous::variable::Variable;
use crate::hir::scopes::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::statement::ParseStatement;
use crate::parser::{ParseSpec, ParseVarSection};
use ast::generated::FuncVariables;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function(
        &mut self,
        func: &ast::generated::FuncDecl,
    ) -> anyhow::Result<PouDecl<'db>> {
        let scope_id = FileScopeId::from((self.file, func.get_id()));
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let variables = func.parse_variables(self)?;
        let statements = func
            .body
            .as_ref()
            .map_or(vec![], |body| match body.children.deref() {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(ref stmts) => stmts
                    .children
                    .iter()
                    .map(|stmt| stmt.to_statement(self))
                    .collect::<anyhow::Result<Vec<_>>>()
                    .unwrap_or_default(),
                _ => vec![],
            });

        let return_type = func
            .return_type
            .as_ref()
            .map(|rt| rt.to_spec(self))
            .transpose()?;

        let name = Ident::from_node(self.db, self.file, func.name.deref())?;
        let usings = self.parse_usings(&func.directives)?;

        let result = PouDecl::new(
            self.db,
            Pou::Function(Function::new(
                self.db,
                variables,
                statements,
                return_type,
                scope_id,
            )),
            func.get_span(),
            name,
            func.name.get_span(),
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

trait ParseVariable<'db> {
    fn parse_variables(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Vec<Variable<'db>>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse_variables(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FuncVariables::InputDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::OutputDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::InOutDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::ExternalVarDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::TempVarDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::VarDecls(decls) => decls.parse(sema, &mut variables)?,
            }
        }

        Ok(variables)
    }
}
