use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{expressions::expression::Expr, expressions::spec::Spec, semantic_index::SemanticIndex},
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
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        Box::new(std::iter::empty())
    }
}
