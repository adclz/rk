use std::ops::Deref;

use crate::hir;
use crate::hir::variable::Variable;
use crate::parser::{Parse, ParseVarSection};
use ast::generated::FuncVariables;
use auto_lsp::anyhow::{self};
use auto_lsp::default::db::{BaseDatabase, File};

impl<'db> Parse<'db> for ast::generated::FuncDecl {
    type Output = hir::function::Function<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let variables = self.parse_variables(db, file)?;
        Ok(hir::function::Function::new(
            db,
            variables))
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
    use auto_lsp::{default::db::FileManager, lsp_types, texter::core::text::Text};

    use super::*;
    use crate::{hir::namespace::Pou, ident::Ident, solver::namespace::{namespaces_in_file, NamespacePath}, RootDatabase};

    #[test]
    fn variables_in_function() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
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
"#.into(),
        );
        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file).unwrap();

        let fn_name = Ident::new(&db, "f".to_string());
        let ns = Ident::new(&db, "nss".to_string());

        let function = namespaces.get_pou(&db as _, NamespacePath::from((&db as _, vec![ns])), fn_name).unwrap().pou(&db);
        
        if let Pou::Function(f) = function {
            assert_eq!(f.variables(&db).len(), 8);
        } else {
            panic!("Not a function");
        }
    }
}
