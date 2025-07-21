use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};

use crate::{
    completions,
    hir::{namespace::Using, statement::Stmt, variable::Variable},
    to_proto::{HirCtx, IterToProto, ProtoAndCtx},
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
        Some(vec![
            completions::snippets::var_input(),
            completions::snippets::var_output(),
            completions::snippets::var_temp(),
            completions::snippets::var(),
        ])
    }
}

impl<'db> IterToProto<'db> for Function<'db> {
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        self.using(ctx.db).iter().map(move |u| (ctx, u as _))
            .chain(self.variables(ctx.db).iter().map(move |v| (ctx, v as _)))
    }
}
