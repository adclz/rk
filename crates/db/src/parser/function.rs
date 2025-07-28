use std::ops::Deref;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::function::Function;
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::scopes::scope::{PouId, Scope, ScopeId, ScopeKind, ScopedPouId, Visibility};
use crate::hir::pous::variable::Variable;
use crate::hir::visibility::Modifiers;
use crate::parser::semantic_index::{SemanticIndexBuilder};
use crate::parser::statement::ParseStatement;
use crate::parser::{ParseVarSection};
use ast::generated::{FbDecl, FbVariables, FuncVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{file::File, BaseDatabase};
use salsa::Accumulator;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function(&mut self, func: &ast::generated::FuncDecl) -> anyhow::Result<PouId> {
        let variables = func.parse_variables(self.db, self.file)?;
        let statements = func
            .body
            .as_ref()
            .map_or(vec![], |body| match body.children.deref() {
                ast::generated::FbDiagram_LadderDiagram_StmtList::StmtList(ref stmts) => stmts
                    .children
                    .iter()
                    .map(|stmt| stmt.to_statement(self.db, self.file))
                    .collect::<anyhow::Result<Vec<_>>>()
                    .unwrap_or_default(),
                _ => vec![],
            });

        let id = ScopeId::from(func.get_id());
        let pou_key = PouId::from(func.get_id());
        let name = Ident::from_node(self.db, self.file, func.name.deref())?;
        let usings = self.parse_usings(&func.directives)?;

        let result =
            Function::new(self.db, variables, statements, self.current_scope);

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(pou_key),
            usings,
            id,
            Visibility::empty(),
            Some(self.current_scope),
        );

        self.pou_keys.insert(
            PouId::from(func.get_id()),
            PouDecl::new(
                self.db,
                Pou::Function(result),
                func.get_span(),
                name,
                func.name.get_span(),
            ),
        );

        self.scope_to_pous.entry(id).or_default().insert(
                name.clone(),
                ScopedPouId(pou_key, self.file),
        );
 
        Ok(pou_key)
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Variable<'db>>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse_variables(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FuncVariables::InputDecls(decls) => decls.parse(db, file, &mut variables)?,
                FuncVariables::OutputDecls(decls) => decls.parse(db, file, &mut variables)?,
                FuncVariables::InOutDecls(decls) => decls.parse(db, file, &mut variables)?,
                FuncVariables::ExternalVarDecls(decls) => decls.parse(db, file, &mut variables)?,
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
    use crate::{
        hir::{interned::{identifier::SpannedIdent, namespace::NamespacePath}, semantic_index::semantic_index}, RootDatabase
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
        let namespaces = semantic_index(&db, file).unwrap();

        let fn_name = SpannedIdent::from_blank(&db, "f");
        let ns = SpannedIdent::from_blank(&db, "nss");

        let ns = NamespacePath::from((&db as _, vec![ns]));

        eprintln!("Namespace: {:?}", namespaces);
    }
}
