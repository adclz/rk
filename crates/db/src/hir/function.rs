
use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};

use crate::{completions, hir::{statement::Stmt, variable::Variable}, to_proto::{IterToProto, ToProto}};

#[salsa::tracked]
pub struct Function<'db> {
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    #[tracked]
    #[no_eq]
    pub statements: Vec<Stmt<'db>>,
}

impl<'db> Function<'db> {
    pub fn completion_ctx(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Vec<CompletionItem>> {
        Some(vec![
            completions::snippets::var_input(),
            completions::snippets::var_output(),
            completions::snippets::var_temp(),
            completions::snippets::var(),
        ])
    }
}

impl<'db> IterToProto<'db> for Function<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.variables(db).iter().map(|v| v as _)
    }
}

