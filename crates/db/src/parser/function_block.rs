use std::ops::Deref;

use ast::generated::FbVariables;
use auto_lsp::anyhow;
use auto_lsp::default::db::{BaseDatabase, File};
use crate::hir::variable::Variable;
use crate::parser::variables::ParseVarSection;
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::FbDecl {
    type Output = hir::function_block::FunctionBlock<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let (
            input_variables,
            output_variables,
            in_out_variables,
            external_variables,
            temp_variables,
            variables,
            retain_variables,
            no_retain_variables,
            loc_partly_variables,
        ) = self.parse_variables(db, file)?;
        Ok(hir::function_block::FunctionBlock::new(
            db,
            input_variables,
            output_variables,
            in_out_variables,
            external_variables,
            temp_variables,
            variables,
            retain_variables,
            no_retain_variables,
            loc_partly_variables,
        ))
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<(
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
    )>;
}

impl<'db> ParseVariable<'db> for ast::generated::FbDecl {
    fn parse_variables(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<(
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
        Vec<Variable<'db>>,
    )> {
        let mut input_variables = vec![];
        let mut output_variables = vec![];
        let mut in_out_variables = vec![];
        let mut external_variables = vec![];
        let mut temp_variables = vec![];
        let mut variables = vec![];
        let mut retain_variables = vec![];
        let mut no_retain_variables = vec![];
        let mut loc_partly_variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FbVariables::FbInputDecls(decls) => decls.parse(db, file, &mut input_variables)?,
                FbVariables::FbOutputDecls(decls) => {
                                decls.parse(db, file, &mut output_variables)?
                            }
                FbVariables::InOutDecls(decls) => decls.parse(db, file, &mut in_out_variables)?,
                FbVariables::ExternalVarDecls(decls) => {
                                decls.parse(db, file, &mut external_variables)?
                            }
                FbVariables::TempVarDecls(decls) => decls.parse(db, file, &mut temp_variables)?,
                FbVariables::VarDecls(decls) => decls.parse(db, file, &mut variables)?,
                FbVariables::LocPartlyVarDecl(loc_partly_var_decl) => loc_partly_var_decl.parse(db, file, &mut loc_partly_variables)?,
                FbVariables::NoRetainVarDecls(no_retain_var_decls) => no_retain_var_decls.parse(db, file, &mut no_retain_variables)?,
                FbVariables::RetainVarDecls(retain_var_decls) => retain_var_decls.parse(db, file, &mut retain_variables)?,
            }
        }
        
        Ok((
            input_variables,
            output_variables,
            in_out_variables,
            external_variables,
            temp_variables,
            variables,
            retain_variables,
            no_retain_variables,
            loc_partly_variables,
        ))
    }
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::{tracked::get_ast, FileManager}, lsp_types, texter::core::text::Text};

    use super::*;
    use crate::{hir::namespace::{Pou, PouDecl}, ident::Ident, solver::{namespaces_in_file, NamespacePath}, RootDatabase};

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
            assert_eq!(f.input_variables(&db).len(), 1);
            assert_eq!(f.output_variables(&db).len(), 1);
            assert_eq!(f.in_out_variables(&db).len(), 1);
            assert_eq!(f.temp_variables(&db).len(), 1);
            assert_eq!(f.external_variables(&db).len(), 1);
            assert_eq!(f.global_variables(&db).len(), 3);
            assert_eq!(f.retain_variables(&db).len(), 1);
            assert_eq!(f.no_retain_variables(&db).len(), 3);
            assert_eq!(f.loc_partly_variables(&db).len(), 1);
        } else {
            panic!("Not a function");
        }
    }
}
