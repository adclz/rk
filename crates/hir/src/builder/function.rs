use crate::Visibility;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{Parse, ParseSpec, ParseVarSection};
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::function::Function;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::scope::ScopeKind;
use ast::generated::FuncVariables;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use ide_diagnostic::IdeDiagnostic;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function(
        &mut self,
        func: &ast::generated::FuncDecl,
    ) -> anyhow::Result<Pou<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let variables = self.parse_func_variables(func);

        let statements = func.body.as_ref().map_or(vec![], |body| {
            match body.cast(self.ast).children.cast(self.ast) {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(stmts) => stmts
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

        let return_type = func
            .return_type
            .as_ref()
            .map(|rt| rt.cast(self.ast).to_spec(self))
            .transpose()?;

        let name = Ident::from_node(self.db, self.file, func.name.cast(self.ast))?;
        let usings = self.parse_usings(&func.directives)?;

        let generics = if let Some(generics) = &func.generic_spec {
            self.parse_generic_params(generics.cast(self.ast))?
        } else {
            vec![]
        };

        let result = Pou::Function(Function::new(
            self.db,
            name,
            func.name.cast(self.ast).into(),
            generics,
            variables,
            statements,
            return_type,
            func.into(),
            scope_id,
        ));

        self.register_node(func.into(), HirNode::PouDecl(result));
        self.register_scope(
            ScopeKind::Pou(result),
            usings,
            scope_id,
            Visibility::empty(),
            previous_scope,
        );

        Ok(result)
    }
}

impl<'db> SemanticIndexBuilder<'db> {
    fn parse_func_variables(&mut self, func: &ast::generated::FuncDecl) -> Vec<VariableDecl<'db>> {
        let mut variables = vec![];

        for variable in func.variables.iter() {
            match variable.cast(self.ast) {
                FuncVariables::ERRVarAccessNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarAccessNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FuncVariables::ERRVarConfigNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarConfigNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FuncVariables::ERRVarLocatedNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarLocatedNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FuncVariables::ERRVarExternalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarExternalNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FuncVariables::ERRVarGlobalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarGlobalNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FuncVariables::InputDecls(decls) => decls.parse(self, &mut variables),
                FuncVariables::OutputDecls(decls) => decls.parse(self, &mut variables),
                FuncVariables::InOutDecls(decls) => decls.parse(self, &mut variables),
                FuncVariables::ExternalVarDecls(decls) => decls.parse(self, &mut variables),
                FuncVariables::TempVarDecls(decls) => decls.parse(self, &mut variables),
                FuncVariables::VarDecls(decls) => decls.parse(self, &mut variables),
            }
        }

        variables
    }
}
