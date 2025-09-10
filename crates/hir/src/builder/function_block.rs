use crate::builder::ParseVarSection;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::statement::ParseStatement;
use crate::check::errors::sem_errors::{AnalysisError};
use crate::check::errors::syntax::SyntaxError;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::hir_def::modifier::Modifier;
use crate::hir_def::pous::function_block::FunctionBlock;
use crate::hir_def::pous::pou::{Pou, PouDecl};
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::scope::{FileScopeId, Scope, ScopeKind, Visibility};
use ast::generated::{FbDecl, FbVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function_block(
        &mut self,
        func: &FbDecl,
    ) -> anyhow::Result<PouDecl<'db>, AnalysisError<'db>> {
        let scope_id = FileScopeId::from((self.db, self.file, func.get_id()));
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

        let mut modifiers = Modifier::empty();
        func.qualifier.as_ref().map(|q| match q.cast(self.ast) {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifier::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifier::FINAL),
        });

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
                self.db, extends, implements, variables, statements, modifiers, scope_id,
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

        self.scope_keys.insert(scope_id, scope);

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

#[cfg(test)]
mod tests {
    use auto_lsp::default::db::{BaseDatabase, file::File};
    use auto_lsp::{default::db::FileManager, lsp_types};
    use db::RootDatabase;

    use crate::hir_def::semantic_index::semantic_index;

    #[test]
    fn variables_in_function_block() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE nss
    FUNCTION_BLOCK f
        VAR_INPUT
            a : INT;
        END_VAR

        VAR_OUTPUT
            b : INT;
        END_VAR

        VAR_IN_OUT
            c: STRING[0];
        END_VAR

        VAR_TEMP
            d : STRING[0];
        END_VAR

        VAR_EXTERNAL
            e : STRING[0];
        END_VAR
  
        // multiple declarations
        VAR
            f, g, h : STRING[0];
        END_VAR

        VAR NON_RETAIN
            i, j, k : STRING[0];
        END_VAR

        VAR RETAIN
            l : STRING[0];
        END_VAR

        VAR 
            head AT %I*: INT; 
        END_VAR
    END_FUNCTION_BLOCK

END_NAMESPACE
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = semantic_index(&db, file);
    }
}
