
use auto_lsp::default::db::BaseDatabase;

use crate::{hir::variable::Variable, to_proto::{IterToProto, ToProto}};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,
}

impl<'db> IterToProto<'db> for Function<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.variables(db).iter().map(|v| v as _)
    }
}
