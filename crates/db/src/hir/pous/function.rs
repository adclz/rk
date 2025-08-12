use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};

use crate::{
    completions,
    hir::{
        expressions::{spec::Spec, statement::Stmt},
        pous::variable::Variable,
        scopes::scope::FileScopeId,
        semantic_index::SemanticIndex,
    },
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    #[tracked]
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    // Statements
    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    #[tracked]
    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    pub scope_id: FileScopeId,
}

impl<'db> Function<'db> {
    pub fn completion_ctx(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let var_completions = vec![
            completions::snippets::var_input(),
            completions::snippets::var_output(),
            completions::snippets::var_temp(),
            completions::snippets::var(),
        ];

        match self.statements(db).first() {
            Some(first_stmt) if first_stmt.span(db).start_byte > offset => {
                // If the first statement starts after the offset, we are in variable declarations
                Some(var_completions)
            }
            // If there are no statements, we are also in variable declarations
            _ => Some(var_completions),
        }
    }
}

impl<'db> IterToProto<'db> for Function<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        let scope = sema.get_scope(self.scope_id(db));

        scope
            .usings
            .iter()
            .flat_map(move |u| u.iter(db, sema))
            .chain(
                self.variables(db)
                    .iter()
                    .flat_map(move |v| v.iter(db, sema)),
            )
    }
}
