use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{expression::Expr, variable::Spec},
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<Expr<'db>>,
}

impl<'db> IterToProto<'db> for DataType<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::empty()
    }
}
