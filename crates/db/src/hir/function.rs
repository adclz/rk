use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};

use crate::{
    completions,
    hir::{namespace::Using, statement::Stmt, variable::Variable},
    to_proto::{ToProto, IterToProto},
};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    #[tracked]
    #[no_eq]
    pub statements: Vec<Stmt<'db>>,
}

impl<'db> Function<'db> {
    pub fn completion_ctx(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let var_completions = vec![
            completions::snippets::var_input(),
            completions::snippets::var_output(),
            completions::snippets::var_temp(),
            completions::snippets::var(),
        ];
        if self.statements(db).is_empty() {
            return Some(var_completions);
        }

        if self.statements(db).first().unwrap().span(db).start_byte > offset {
            return Some(var_completions);
        }
        None
        
    }
}

impl<'db> IterToProto<'db> for Function<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.using(db).iter().flat_map(move |u| u.iter(db))
            .chain(self.variables(db).iter().flat_map(move |v| v.iter(db)))
    }
}
