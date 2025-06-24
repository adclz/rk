
use auto_lsp::default::db::BaseDatabase;

use crate::{hir::variable::Variable, to_proto::{IterToProto, SymbolInfo, SymbolInfoBuilder, ToProto}};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    #[returns(ref)]
    pub input_variables: Vec<Variable<'db>>,
    #[returns(ref)]
    pub output_variables: Vec<Variable<'db>>,
    #[returns(ref)]
    pub in_out_variables: Vec<Variable<'db>>,
    #[returns(ref)]
    pub temp_variables: Vec<Variable<'db>>,
    #[returns(ref)]
    pub external_variables: Vec<Variable<'db>>,
    #[returns(ref)]
    pub global_variables: Vec<Variable<'db>>,
}

impl<'db> IterToProto<'db> for Function<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>> {
        self.input_variables(db).iter().map(|v| v.symbol_info(db))
            .chain(self.output_variables(db).iter().map(|v| v.symbol_info(db)))
            .chain(self.in_out_variables(db).iter().map(|v| v.symbol_info(db)))
            .chain(self.temp_variables(db).iter().map(|v| v.symbol_info(db)))
            .chain(self.external_variables(db).iter().map(|v| v.symbol_info(db)))
            .chain(self.global_variables(db).iter().map(|v| v.symbol_info(db)))
    }
}