use crate::{
    hir::{namespace::Using, variable::Variable, visibility::Modifiers},
    solver::fq_name::SpannedPath,
    to_proto::{HirCtx, IterToProto, ProtoAndCtx},
};

#[salsa::tracked(debug)]
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
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        self.using(ctx.db).iter().map(move |u| (ctx, u as _))
            .chain(self.variables(ctx.db).iter().map(move |v| (ctx, v as _)))
    }
}
