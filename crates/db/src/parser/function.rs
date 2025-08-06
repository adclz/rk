use std::ops::Deref;

use crate::hir::interned::identifier::Ident;
use crate::hir::pous::function::Function;
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::pous::variable::Variable;
use crate::hir::scopes::scope::{PouId, Scope, ScopeKind, Visibility};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::statement::ParseStatement;
use crate::parser::{ParseSpec, ParseVarSection};
use ast::generated::FuncVariables;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function(&mut self, func: &ast::generated::FuncDecl) -> anyhow::Result<PouId> {
        let variables = func.parse_variables(self)?;
        let statements = func
            .body
            .as_ref()
            .map_or(vec![], |body| match body.children.deref() {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(ref stmts) => stmts
                    .children
                    .iter()
                    .map(|stmt| stmt.to_statement(self))
                    .collect::<anyhow::Result<Vec<_>>>()
                    .unwrap_or_default(),
                _ => vec![],
            });

        let return_type = func
            .return_type
            .as_ref()
            .map(|rt| rt.to_spec(self))
            .transpose()?;

        let (id, pou_key, file_id) = self.create_pou_id(func);

        let name = Ident::from_node(self.db, self.file, func.name.deref())?;
        let usings = self.parse_usings(&func.directives)?;

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(pou_key),
            usings,
            id,
            Visibility::empty(),
            Some(self.current_scope),
        );

        self.scope_keys.insert(id, scope);

        self.pou_keys.insert(
            pou_key,
            PouDecl::new(
                self.db,
                Pou::Function(Function::new(
                    self.db,
                    variables,
                    statements,
                    return_type,
                    self.current_scope,
                )),
                func.get_span(),
                name,
                func.name.get_span(),
                file_id,
                self.current_scope,
            ),
        );

        Ok(pou_key)
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Vec<Variable<'db>>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse_variables(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FuncVariables::InputDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::OutputDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::InOutDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::ExternalVarDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::TempVarDecls(decls) => decls.parse(sema, &mut variables)?,
                FuncVariables::VarDecls(decls) => decls.parse(sema, &mut variables)?,
            }
        }

        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        hir::{pous::pou::Pou, semantic_index::semantic_index},
        RootDatabase,
    };
    use auto_lsp::{
        default::db::{file::File, BaseDatabase, FileManager},
        lsp_types,
    };

    #[test]
    fn variables_in_function() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
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
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let sema = semantic_index(&db, file);

        let first_pou = sema.pou_keys.iter().next().unwrap();
        let pou_decl = first_pou.1;

        assert_eq!(pou_decl.name(&db).text(&db), "f");
        match pou_decl.pou(&db) {
            Pou::Function(func) => {
                assert_eq!(func.variables(&db).len(), 8);
            }
            _ => panic!("Expected a function POU"),
        }
    }
}
