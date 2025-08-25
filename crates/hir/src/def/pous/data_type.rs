use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        expressions::{expression::InitExpr, spec::Spec},
        scope::FileScopeId,
        semantic_index::SemanticIndex,
    },
    to_proto::{ToProto},
};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<InitExpr<'db>>,

    pub scope_id: FileScopeId,
}
