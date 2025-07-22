use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{expression::Expr, variable::Spec},
    to_proto::{HirCtx, IterToProto, ProtoAndCtx},
};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<Expr<'db>>,
}

impl<'db> IterToProto<'db> for DataType<'db> {
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        Box::new(std::iter::empty())
    }
}
