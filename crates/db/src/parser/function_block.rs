use std::ops::Deref;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir;
use crate::hir::variable::Variable;
use crate::hir::visibility::Modifiers;
use crate::parser::namespace::ParseUsing;
use crate::parser::{Parse, ParseVarSection};
use crate::solver::fq_name::SpannedNamespaceAccess;
use ast::generated::FbVariables;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{file::File, BaseDatabase};
use salsa::Accumulator;

impl<'db> Parse<'db> for ast::generated::FbDecl {
    type Output = hir::function_block::FunctionBlock<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let variables = self.parse_variables(db, file)?;

        let extends = self
            .extends
            .as_ref()
            .map(|e| SpannedNamespaceAccess::new(db, file, e))
            .transpose()?;

        let implements = self
            .implements
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::new(db, file, i))
                    .collect()
            })
            .transpose()?; 

        self.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.deref() {
                Error::ERRExtendsMultipleTimes(err) => {
                    let diag = diag()
                        .message("EXTENDS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                }, 
                Error::ERRImplementsBeforeExtends(err) => {
                    let diag = diag()
                        .message("IMPLEMENTS can only be defined after EXTENDS".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    let diag = diag()
                        .message("IMPLEMENTS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                },
            } 
        });

        let mut modifiers = Modifiers::empty();
        self.qualifier.as_ref().map(|q| match q.deref() {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifiers::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifiers::FINAL),
        });

        let using = self.directives.parse_using(db, file)?;

        Ok(hir::function_block::FunctionBlock::new(
            db, extends, implements, using, variables, modifiers,
        ))
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Variable<'db>>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FbDecl {
    fn parse_variables(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FbVariables::FbInputDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::FbOutputDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::InOutDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::ExternalVarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::TempVarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::VarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::LocPartlyVarDecl(loc_partly_var_decl) => {
                    loc_partly_var_decl.parse(db, file, &mut variables)?
                }
                FbVariables::NoRetainVarDecls(no_retain_var_decls) => {
                    no_retain_var_decls.parse(db, file, &mut variables)?
                }
                FbVariables::RetainVarDecls(retain_var_decls) => {
                    retain_var_decls.parse(db, file, &mut variables)?
                }
            }
        }

        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types};

    use super::*;
    use crate::{
        hir::namespace::{Pou, PouResult},
        ident::Ident,
        solver::namespace::{namespaces_in_file, NamespacePath},
        RootDatabase,
    };

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
        let namespaces = namespaces_in_file(&db, file).unwrap();

        let fn_name = Ident::new(&db, "f".to_string());
        let ns = Ident::new(&db, "nss".to_string());

        let ns = NamespacePath::from((&db as _, vec![ns]));
        let function = namespaces.get_pou(&db as _, ns, ns, fn_name);

        let PouResult::Found(pou) = function else {
            panic!("Not a function block");
        };

        if let Pou::FunctionBlock(f) = pou.pou(&db) {
            assert_eq!(f.variables(&db).len(), 13);
        } else {
            panic!("Not a function block");
        }
    }
}
