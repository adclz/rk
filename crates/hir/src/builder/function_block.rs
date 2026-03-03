use crate::builder::Parse;
use crate::builder::{ParseSpec, ParseVarSection};
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::function_block::FunctionBlock;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::scope::ScopeKind;
use crate::{Modifier, Visibility};
use ast::generated::{FbDecl, FbVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use ide_diagnostic::IdeDiagnostic;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function_block(
        &mut self,
        func: &FbDecl,
    ) -> anyhow::Result<Pou<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let variables = self.parse_fb_variables(func);

        let extends = func.extends.as_ref().and_then(|e| {
            let spec = e.cast(self.ast).to_spec(self);
            self.try_parse(spec)
        });

        let implements = func
            .implements
            .as_ref()
            .map(|i| {
                i.cast(self.ast)
                    .children
                    .iter()
                    .filter_map(|i| {
                        let spec = i.cast(self.ast).to_spec(self);
                        self.try_parse(spec)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        func.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRFbVariablesAfterMethod_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
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
                Error::ERRFbVariablesAfterMethod(err) => {
                    self.errors.push(SyntaxError::FbVariablesAfterMethod(err.get_span()).to_diagnostic(self.db));
                },
            }
        });

        let statements = func.body.as_ref().map_or(vec![], |body| {
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
        let usings = self.parse_usings(&func.directives);
        let usings = self.parse_or_default(usings);

        let generics = if let Some(generics) = &func.generic_spec {
            self.parse_generic_params(generics.cast(self.ast))?
        } else {
            vec![]
        };

        let result = Pou::FunctionBlock(FunctionBlock::new(
            self.db,
            name,
            func.name.cast(self.ast).into(),
            generics,
            extends,
            implements,
            variables,
            self.parse_methods(&func.method),
            statements,
            modifiers,
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
    fn parse_fb_variables(&mut self, func: &ast::generated::FbDecl) -> Vec<VariableDecl<'db>> {
        let mut variables = vec![];

        for variable in func.variables.iter() {
            match variable.cast(self.ast) {
                FbVariables::ERRVarAccessNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarAccessNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FbVariables::ERRVarConfigNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarConfigNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FbVariables::ERRVarLocatedNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarLocatedNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FbVariables::ERRVarGlobalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarGlobalNotAllowed(err.get_span()).to_diagnostic(self.db),
                    );
                }
                FbVariables::FbInputDecls(decls) => decls.parse(self, &mut variables),
                FbVariables::FbOutputDecls(decls) => decls.parse(self, &mut variables),
                FbVariables::InOutDecls(decls) => decls.parse(self, &mut variables),
                FbVariables::ExternalVarDecls(decls) => decls.parse(self, &mut variables),
                FbVariables::TempVarDecls(decls) => decls.parse(self, &mut variables),
                FbVariables::VarDecls(decls) => decls.parse(self, &mut variables),
                FbVariables::LocPartlyVarDecl(loc_partly_var_decl) => {
                    loc_partly_var_decl.parse(self, &mut variables)
                }
                FbVariables::NoRetainVarDecls(no_retain_var_decls) => {
                    no_retain_var_decls.parse(self, &mut variables)
                }
                FbVariables::RetainVarDecls(retain_var_decls) => {
                    retain_var_decls.parse(self, &mut variables)
                }
            }
        }

        variables
    }
}
