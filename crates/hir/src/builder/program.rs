use std::sync::Arc;

use auto_lsp::anyhow;

use crate::{
    Visibility,
    builder::{
        ParseVarSection, semantic_index::SemanticIndexBuilder, statement::ParseStatement,
        variables::ParseProgDecl,
    },
    hir_def::{
        interned::identifier::Ident,
        pous::variable::VariableDecl,
        program::{ProgAccessDecl, ProgramDecl},
        scope::{Scope, ScopeKind},
    },
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_program(
        &mut self,
        program: &ast::generated::ProgDecl,
    ) -> anyhow::Result<ProgramDecl<'db>> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let (prog_access_decls, variables) = program.parse_variables(self);

        let name = Ident::from_node(self.db, self.file, program.name.cast(self.ast))?;

        let statements = program.body.as_ref().map_or(vec![], |body| {
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

        let program = ProgramDecl::new(
            self.db,
            name,
            program.name.cast(self.ast).into(),
            prog_access_decls,
            variables,
            statements,
            program.into(),
            scope_id,
        );

        let scope = Scope::new(
            self.file,
            ScopeKind::Program(program),
            vec![],
            scope_id,
            Visibility::empty(),
            Some(previous_scope),
        );

        self.scope_keys
            .insert(scope_id.scope(self.db), Arc::new(scope));

        Ok(program)
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> (Vec<ProgAccessDecl<'db>>, Vec<VariableDecl<'db>>);
}

impl<'db> ParseVariable<'db> for ast::generated::ProgDecl {
    fn parse_variables(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> (Vec<ProgAccessDecl<'db>>, Vec<VariableDecl<'db>>) {
        let mut prog_decls = vec![];
        let mut variables = vec![];
        type ProgVariables = ast::generated::ExternalVarDecls_GlobalVarDecls_InOutDecls_InputDecls_LocPartlyVarDecl_LocVarDecls_NoRetainVarDecls_OutputDecls_ProgAccessDecls_RetainVarDecls_TempVarDecls_VarDecls;

        for variable in self.declarations.iter() {
            match variable.cast(sema.ast) {
                ProgVariables::ProgAccessDecls(decls) => decls.parse(sema, &mut prog_decls),
                ProgVariables::GlobalVarDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::InputDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::OutputDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::InOutDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::ExternalVarDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::TempVarDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::LocVarDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::VarDecls(decls) => decls.parse(sema, &mut variables),
                ProgVariables::LocPartlyVarDecl(loc_partly_var_decl) => {
                    loc_partly_var_decl.parse(sema, &mut variables)
                }
                ProgVariables::NoRetainVarDecls(no_retain_var_decls) => {
                    no_retain_var_decls.parse(sema, &mut variables)
                }
                ProgVariables::RetainVarDecls(retain_var_decls) => {
                    retain_var_decls.parse(sema, &mut variables)
                }
            }
        }

        (prog_decls, variables)
    }
}
