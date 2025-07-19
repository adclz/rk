use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{namespace::Using, variable::Variable, visibility::Modifiers},
    solver::fq_name::SpannedPath,
    to_proto::{HirCtx, IterToProto, ToProto},
};

#[salsa::tracked]
pub struct FunctionBlock<'db> {
    pub extends: Option<SpannedPath>,

    pub implements: Option<Vec<SpannedPath>>,

    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    pub modifiers: Modifiers,
}

impl<'db> IterToProto<'db> for FunctionBlock<'db> {
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = &'db (dyn ToProto<'db> + 'db)> {
        self.using(ctx.db).iter().map(|u| u as _)
            .chain(self.variables(ctx.db).iter().map(|v| v as _))
    }
}
