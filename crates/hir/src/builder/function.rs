use std::sync::Arc;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::statement::ParseStatement;
use crate::builder::{ParseSpec, ParseVarSection};
use crate::check::errors::analysis_error::AnalysisError;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::function::Function;
use crate::hir_def::pous::pou::{Pou};
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::scope::{Scope, ScopeKind};
use crate::hir_def::visibility::Visibility;
use ast::generated::FuncVariables;
use auto_lsp::anyhow;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function(
        &mut self,
        func: &ast::generated::FuncDecl,
    ) -> anyhow::Result<Pou<'db>, AnalysisError<'db>> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let variables = func.parse_variables(self);

        let statements = func.body.as_ref().map_or(vec![], |body| {
            match body.cast(self.ast).children.cast(self.ast) {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(stmts) => stmts
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

        let return_type = func
            .return_type
            .as_ref()
            .map(|rt| rt.cast(self.ast).to_spec(self))
            .transpose()?;

        let name = Ident::from_node(self.db, self.file, func.name.cast(self.ast))?;
        let usings = self.parse_usings(&func.directives)?;

        let result = 
            Pou::Function(Function::new(
                self.db,
                name,
                            func.name.cast(self.ast).into(),
                variables,
                statements,
                return_type,
                func.into(),
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

        self.scope_keys.insert(scope_id.scope(self.db), Arc::new(scope));

        Ok(result)
    }
}

trait ParseVariable<'db> {
    fn parse_variables(&self, sema: &mut SemanticIndexBuilder<'db>) -> Vec<VariableDecl<'db>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse_variables(&self, sema: &mut SemanticIndexBuilder<'db>) -> Vec<VariableDecl<'db>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.cast(sema.ast) {
                FuncVariables::InputDecls(decls) => decls.parse(sema, &mut variables),
                FuncVariables::OutputDecls(decls) => decls.parse(sema, &mut variables),
                FuncVariables::InOutDecls(decls) => decls.parse(sema, &mut variables),
                FuncVariables::ExternalVarDecls(decls) => decls.parse(sema, &mut variables),
                FuncVariables::TempVarDecls(decls) => decls.parse(sema, &mut variables),
                FuncVariables::VarDecls(decls) => decls.parse(sema, &mut variables),
            }
        }

        variables
    }
}
