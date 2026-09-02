use auto_lsp::core::ast::AstNode;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    builder::{
        Parse, ParseVarSection,
        semantic_index::SemanticIndexBuilder,
        variables::ParseProgDecl,
    },
    check::errors::{ToIdeDiagnostic, e0_syntax::SyntaxError},
    hir_def::{
        hir_node::HirNode,
        interned::identifier::Ident,
        pous::variable::VariableDecl,
        program::{ProgAccessDecl, ProgramDecl},
        scope::ScopeKind,
    },
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_program(
        &mut self,
        program: &ast::generated::ProgDecl,
    ) -> Result<ProgramDecl<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let (prog_access_decls, variables) = self.parse_prog_variables(program);

        let name = Ident::from_node(self.db, self.file, program.name.cast(self.ast))?;

        let statements = program.body.as_ref().map_or(vec![], |body| {
            match body.cast(self.ast).children.cast(self.ast) {
                ast::generated::SFC_FbDiagram_LadderDiagram_StmtList::StmtList(stmts) => stmts
                    .children
                    .iter()
                    .filter_map(|stmt| {
                        let r = stmt.cast(self.ast).parse(self);
                        self.try_parse(r)
                    })
                    .collect(),
                _ => vec![],
            }
        });

        let pragmas = self.parse_pou_pragmas(&program.pragmas);

        let result = ProgramDecl::new(
            self.db,
            name,
            pragmas,
            program.name.cast(self.ast).into(),
            prog_access_decls,
            variables,
            statements,
            program.into(),
            scope_id,
        );

        self.register_node(program.into(), HirNode::Program(result));
        self.register_scope(
            ScopeKind::Program(result),
            vec![],
            scope_id,
            previous_scope,
);

        Ok(result)
    }
}

impl<'db> SemanticIndexBuilder<'db> {
    fn parse_prog_variables(
        &mut self,
        program: &ast::generated::ProgDecl,
    ) -> (Vec<ProgAccessDecl<'db>>, Vec<VariableDecl<'db>>) {
        let mut prog_decls = vec![];
        let mut variables = vec![];
        type ProgVariables = ast::generated::ERRVarGlobalNotAllowed_ExternalVarDecls_InOutDecls_InputDecls_LocPartlyVarDecl_NoRetainVarDecls_OutputDecls_ProgAccessDecls_RetainVarDecls_TempVarDecls_VarDecls;

        for variable in program.declarations.iter() {
            match variable.cast(self.ast) {
                ProgVariables::ProgAccessDecls(decls) => decls.parse(self, &mut prog_decls),
                // Globals are application-scoped: they belong to a
                // CONFIGURATION, not to a POU.
                ProgVariables::ERRVarGlobalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarGlobalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ProgVariables::InputDecls(decls) => decls.parse(self, &mut variables),
                ProgVariables::OutputDecls(decls) => decls.parse(self, &mut variables),
                ProgVariables::InOutDecls(decls) => decls.parse(self, &mut variables),
                ProgVariables::ExternalVarDecls(decls) => decls.parse(self, &mut variables),
                ProgVariables::TempVarDecls(decls) => decls.parse(self, &mut variables),
                ProgVariables::VarDecls(decls) => decls.parse(self, &mut variables),
                ProgVariables::LocPartlyVarDecl(loc_partly_var_decl) => {
                    loc_partly_var_decl.parse(self, &mut variables)
                }
                ProgVariables::NoRetainVarDecls(no_retain_var_decls) => {
                    no_retain_var_decls.parse(self, &mut variables)
                }
                ProgVariables::RetainVarDecls(retain_var_decls) => {
                    retain_var_decls.parse(self, &mut variables)
                }
            }
        }

        (prog_decls, variables)
    }
}
