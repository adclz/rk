use auto_lsp::default::db::BaseDatabase;

use crate::{hir::variable::Variable, to_proto::{IterToProto, SymbolInfo, ToProto}};


#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,
}


impl<'db> IterToProto<'db> for FunctionBlock<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>> {
        self.variables(db).iter().map(|v| v.symbol_info(db))
    }
}
