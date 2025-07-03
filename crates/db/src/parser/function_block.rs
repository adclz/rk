use std::ops::Deref;

use ast::generated::FbVariables;
use auto_lsp::anyhow;
use auto_lsp::default::db::{BaseDatabase, File};
use crate::hir::expression::Expr;
use crate::hir::variable::Variable;
use crate::parser::{Parse, ParseVarSection};
use crate::hir;

impl<'db> Parse<'db> for ast::generated::FbDecl {
    type Output = hir::function_block::FunctionBlock<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let variables = self.parse_variables(db, file)?;

        let extends = self
            .extends
            .as_ref()
            .map(|e| Expr::new_target(db, file, e))
            .transpose()?;

        let implements = self
            .implements
            .as_ref()
            .map(|i| i.children.iter().map(|i| Expr::new_target(db, file, i)).collect())
            .transpose()?;

        Ok(hir::function_block::FunctionBlock::new(
            db,
            extends,
            implements,
            variables,
        ))
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<
        Vec<Variable<'db>>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FbDecl {
    fn parse_variables(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result< 
        Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FbVariables::FbInputDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::FbOutputDecls(decls) => {
                                decls.parse(db, file, &mut variables)?
                            }
                FbVariables::InOutDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::ExternalVarDecls(decls) => {
                                decls.parse(db, file, &mut variables)?
                            }
                FbVariables::TempVarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::VarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::LocPartlyVarDecl(loc_partly_var_decl) => loc_partly_var_decl.parse(db, file, &mut variables)?,
                FbVariables::NoRetainVarDecls(no_retain_var_decls) => no_retain_var_decls.parse(db, file, &mut variables)?,
                FbVariables::RetainVarDecls(retain_var_decls) => retain_var_decls.parse(db, file, &mut variables)?,
            }
        }
        
        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types, texter::core::text::Text};

    use super::*;
    use crate::{hir::namespace::Pou, ident::Ident, solver::{namespaces_in_file, NamespacePath}, RootDatabase};

    #[test]
    fn variables_in_function_block() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
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
        
        if let Pou::FunctionBlock(f) = function {
            assert_eq!(f.variables(&db).len(), 12);
        } else {
            panic!("Not a function");
        }
    }
}
