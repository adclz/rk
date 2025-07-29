use std::ops::Deref;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::function_block::FunctionBlock;
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::pous::variable::Variable;
use crate::hir::scopes::scope::{PouId, Scope, ScopeId, ScopeKind, ScopedPouId, Visibility};
use crate::hir::visibility::Modifiers;
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::ParseVarSection;
use ast::generated::{FbDecl, FbVariables};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use salsa::Accumulator;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_function_block(&mut self, func: &FbDecl) -> anyhow::Result<PouId> {
        let variables = func.parse_variables(self)?;

        let extends = func
            .extends
            .as_ref()
            .map(|e| SpannedNamespaceAccess::from_ast(self.db, self.file, e))
            .transpose()?;

        let implements = func
            .implements
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::from_ast(self.db, self.file, i))
                    .collect()
            })
            .transpose()?;

        func.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.deref() {
                Error::ERRExtendsMultipleTimes(err) => {
                    let diag = diag()
                        .message("EXTENDS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    let diag = diag()
                        .message("IMPLEMENTS can only be defined after EXTENDS".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    let diag = diag()
                        .message("IMPLEMENTS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                },
            }
        });

        let mut modifiers = Modifiers::empty();
        func.qualifier.as_ref().map(|q| match q.deref() {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifiers::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifiers::FINAL),
        });

        let id = ScopeId::from(func.get_id());
        let pou_key = PouId::from(func.get_id());
        let name = Ident::from_node(self.db, self.file, func.name.deref())?;
        let usings = self.parse_usings(&func.directives)?;

        let result = FunctionBlock::new(
            self.db,
            extends,
            implements,
            variables,
            modifiers,
            self.current_scope,
        );

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
                Pou::FunctionBlock(result),
                func.get_span(),
                name,
                func.name.get_span(),
            ),
        );

        self.scope_to_pous
            .entry(id)
            .or_default()
            .insert(name.clone(), ScopedPouId(pou_key, self.file));

        Ok(pou_key)
    }
}

trait ParseVariable<'db> {
    fn parse_variables(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Vec<Variable<'db>>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FbDecl {
    fn parse_variables(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Vec<Variable<'db>>> {
        let mut variables = vec![];

        for variable in self.variables.iter() {
            match variable.deref() {
                FbVariables::FbInputDecls(decls) => decls.parse(sema, &mut variables)?,
                FbVariables::FbOutputDecls(decls) => decls.parse(sema, &mut variables)?,
                FbVariables::InOutDecls(decls) => decls.parse(sema, &mut variables)?,
                FbVariables::ExternalVarDecls(decls) => decls.parse(sema, &mut variables)?,
                FbVariables::TempVarDecls(decls) => decls.parse(sema, &mut variables)?,
                FbVariables::VarDecls(decls) => decls.parse(sema, &mut variables)?,
                FbVariables::LocPartlyVarDecl(loc_partly_var_decl) => {
                    loc_partly_var_decl.parse(sema, &mut variables)?
                }
                FbVariables::NoRetainVarDecls(no_retain_var_decls) => {
                    no_retain_var_decls.parse(sema, &mut variables)?
                }
                FbVariables::RetainVarDecls(retain_var_decls) => {
                    retain_var_decls.parse(sema, &mut variables)?
                }
            }
        }

        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types};
    use auto_lsp::default::db::{file::File, BaseDatabase};
    
    use crate::{
        hir::{
            interned::{identifier::SpannedIdent, namespace::NamespacePath},
            semantic_index::semantic_index,
        },
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
        let namespaces = semantic_index(&db, file).unwrap();

        let fn_name = SpannedIdent::from_blank(&db, "f");
        let ns = SpannedIdent::from_blank(&db, "nss");

        let ns = NamespacePath::from((&db as _, vec![ns]));
    }
}
