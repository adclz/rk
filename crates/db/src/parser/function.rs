use std::ops::Deref;
use std::vec;

use crate::hir;
use crate::hir::variable::Variable;
use crate::parser::namespace::ParseUsing;
use crate::parser::statement::ParseStatement;
use crate::parser::{Parse, ParseVarSection};
use ast::generated::FuncVariables;
use auto_lsp::anyhow::{self};
use auto_lsp::default::db::{BaseDatabase, file::File};

impl<'db> Parse<'db> for ast::generated::FuncDecl {
    type Output = hir::function::Function<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let variables = self.parse_variables(db, file)?;
        let statements = self.body
            .as_ref()
            .map_or(vec![], |body| match body.children.deref() {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(ref stmts) => {
                    stmts.children.iter().map(|stmt| stmt.to_statement(db, file)).collect::<anyhow::Result<Vec<_>>>().unwrap_or_default()
                }
                _ => vec![],
            });

        let using = self.directives.parse_using(db, file)?;
    
        Ok(hir::function::Function::new(
            db,
            using,
            variables,
            statements
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

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse_variables(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FuncVariables::InputDecls(decls) => decls.parse(db, file, &mut variables)?,
                FuncVariables::OutputDecls(decls) => {
                    decls.parse(db, file, &mut variables)?
                }
                FuncVariables::InOutDecls(decls) => decls.parse(db, file, &mut variables)?,
                FuncVariables::ExternalVarDecls(decls) => {
                    decls.parse(db, file, &mut variables)?
                }
                FuncVariables::TempVarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FuncVariables::VarDecls(decls) => decls.parse(db, file, &mut variables)?,
            }
        }
        
        Ok(variables)
    }
}


#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types};

    use super::*;
    use crate::{hir::namespace::{Pou, PouResult}, ident::Ident, solver::namespace::{namespaces_in_file, NamespacePath}, RootDatabase};

    #[test]
    fn variables_in_function() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = 
            r#"
NAMESPACE nss
    FUNCTION f
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
    END_FUNCTION

END_NAMESPACE
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call().unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file).unwrap();

        let fn_name = Ident::new(&db, "f".to_string());
        let ns = Ident::new(&db, "nss".to_string());

        let ns = NamespacePath::from((&db as _, vec![ns]));
        let function = namespaces.get_pou(&db as _, ns,  ns, fn_name);
        
        let PouResult::Found(pou) = function else {
            panic!("Not a function");
        };

        if let Pou::Function(f) = pou.pou(&db) {
            assert_eq!(f.variables(&db).len(), 8);
        } else {
            panic!("Not a function");
        }
    }
}
